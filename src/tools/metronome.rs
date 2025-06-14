use wasm_bindgen::prelude::*;
use web_sys::{AudioContext, AudioNode, HtmlCanvasElement};
use wasm_bindgen::JsCast;
use yew::prelude::*;
use gloo_timers::callback::Interval;
use js_sys::Date;

// 인라인 스타일 제거

// 박자 정보를 나타내는 열거형
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum TimeSignature {
    FourFour,    // 4/4
    ThreeFour,   // 3/4
    TwoFour,     // 2/4
    SixEight,    // 6/8
    NineEight,   // 9/8
    TwelveEight, // 12/8
}

impl TimeSignature {
    // 박자의 상단 숫자 (박의 개수) 반환
    fn beats_per_measure(&self) -> u8 {
        match self {
            TimeSignature::FourFour => 4,
            TimeSignature::ThreeFour => 3,
            TimeSignature::TwoFour => 2,
            TimeSignature::SixEight => 6,
            TimeSignature::NineEight => 9,
            TimeSignature::TwelveEight => 12,
        }
    }
    
    // 박자의 하단 숫자 (음표 단위) 반환
    fn beat_unit(&self) -> u8 {
        match self {
            TimeSignature::FourFour | TimeSignature::ThreeFour | TimeSignature::TwoFour => 4,
            TimeSignature::SixEight | TimeSignature::NineEight | TimeSignature::TwelveEight => 8,
        }
    }
    
    // 박자 표시 문자열 반환
    fn display_str(&self) -> String {
        match self {
            TimeSignature::FourFour => "4/4".to_string(),
            TimeSignature::ThreeFour => "3/4".to_string(),
            TimeSignature::TwoFour => "2/4".to_string(),
            TimeSignature::SixEight => "6/8".to_string(),
            TimeSignature::NineEight => "9/8".to_string(),
            TimeSignature::TwelveEight => "12/8".to_string(),
        }
    }
}

// 음표 단위를 나타내는 열거형
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum NoteUnit {
    Quarter,     // 4분 음표
    Eighth,      // 8분 음표
    Triplet,     // 셋잇단 음표
    Sixteenth,   // 16분 음표
}

impl NoteUnit {
    // 음표 단위당 클릭 수 반환
    fn clicks_per_beat(&self) -> u8 {
        match self {
            NoteUnit::Quarter => 1,
            NoteUnit::Eighth => 2,
            NoteUnit::Triplet => 3,
            NoteUnit::Sixteenth => 4,
        }
    }
    
    // 음표 단위 표시 문자열 반환
    fn display_str(&self) -> String {
        match self {
            NoteUnit::Quarter => "Quarter Note (1/4)".to_string(),
            NoteUnit::Eighth => "Eighth Note (1/8)".to_string(),
            NoteUnit::Triplet => "Triplet (1/3)".to_string(),
            NoteUnit::Sixteenth => "Sixteenth Note (1/16)".to_string(),
        }
    }
}

// 메트로놈 컴포넌트의 메시지 정의
pub enum MetronomeMsg {
    Start,
    Stop,
    SetBpm(u32),
    SetTimeSignature(TimeSignature),
    SetNoteUnit(NoteUnit),
    Tick,
    ToggleSound,
    UpdateCanvas,
    TapTempo,
    ToggleAccent,
}

// 메트로놈 컴포넌트의 상태 정의
pub struct Metronome {
    bpm: u32,
    time_signature: TimeSignature,
    note_unit: NoteUnit,
    is_playing: bool,
    current_beat: u32,
    current_click: u32,
    beat_count: u32,
    interval: Option<Interval>,
    canvas_ref: NodeRef,
    sound_enabled: bool,
    audio_ctx: Option<AudioContext>,
    last_update_time: f64,
    total_clicks: u32,
    tap_times: Vec<f64>,
    accent_enabled: bool,
}

impl Component for Metronome {
    type Message = MetronomeMsg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            bpm: 120,
            time_signature: TimeSignature::FourFour,
            note_unit: NoteUnit::Quarter,
            is_playing: false,
            current_beat: 0,
            current_click: 0,
            beat_count: 0,
            interval: None,
            canvas_ref: NodeRef::default(),
            sound_enabled: true,
            audio_ctx: None,
            last_update_time: 0.0,
            total_clicks: 0,
            tap_times: Vec::new(),
            accent_enabled: true,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            MetronomeMsg::Start => {
                if self.is_playing {
                    return false;
                }
                
                self.is_playing = true;
                self.current_beat = 0;
                self.current_click = 0;
                self.total_clicks = 0;
                
                // 오디오 컨텍스트 초기화
                if self.sound_enabled {
                    if self.audio_ctx.is_none() {
                        // 오디오 컨텍스트가 없으면 새로 생성
                        match AudioContext::new() {
                            Ok(context) => {
                                self.audio_ctx = Some(context);
                                web_sys::console::log_1(&"오디오 컨텍스트 생성 성공".into());
                            },
                            Err(err) => {
                                web_sys::console::error_1(&format!("오디오 컨텍스트 생성 실패: {:?}", err).into());
                            }
                        }
                    } else if let Some(context) = &self.audio_ctx {
                        // 이미 컨텍스트가 있는 경우 재개
                        if let Err(err) = context.resume() {
                            web_sys::console::error_1(&format!("오디오 컨텍스트 재개 실패: {:?}", err).into());
                        } else {
                            web_sys::console::log_1(&"오디오 컨텍스트 재개됨".into());
                        }
                    }
                }
                
                // 타이머 인터벌 계산 (밀리초 단위)
                let note_unit_clicks = self.note_unit.clicks_per_beat() as u32;
                let beats_per_minute = self.bpm;
                let beat_time_ms = 60000 / beats_per_minute;
                let click_time_ms = beat_time_ms / note_unit_clicks;
                
                // 초기 시간 설정
                self.last_update_time = Date::now();
                
                // 첫 박자 소리 즉시 재생 (첫 번째 박자이므로 true)
                if self.sound_enabled {
                    self.play_click(true);
                }
                
                // 메트로놈 틱 인터벌 설정
                let link = ctx.link().clone();
                let interval = Interval::new(click_time_ms as u32, move || {
                    link.send_message(MetronomeMsg::Tick);
                });
                
                self.interval = Some(interval);
                
                // 캔버스 업데이트 인터벌 설정 (60fps에 가깝게)
                let canvas_link = ctx.link().clone();
                let canvas_interval = Interval::new(16, move || {
                    canvas_link.send_message(MetronomeMsg::UpdateCanvas);
                });
                
                // 별도로 저장하지 않고 drop 방지를 위해 forget
                canvas_interval.forget();
                
                true
            },
            
            MetronomeMsg::Stop => {
                if !self.is_playing {
                    return false;
                }
                
                self.is_playing = false;
                self.interval = None;
                
                // 오디오 컨텍스트 일시 중지
                if let Some(context) = &self.audio_ctx {
                    let _ = context.suspend();
                }
                
                true
            },
            
            MetronomeMsg::SetBpm(bpm) => {
                if bpm < 30 || bpm > 300 {
                    return false;
                }
                
                // BPM 값 업데이트
                self.bpm = bpm;
                
                // 재생 중인 경우 인터벌 재설정
                if self.is_playing {
                    // 기존 인터벌 제거
                    self.interval = None;
                    
                    // 새 타이머 인터벌 계산 (밀리초 단위)
                    let note_unit_clicks = self.note_unit.clicks_per_beat() as u32;
                    let beats_per_minute = self.bpm;
                    let beat_time_ms = 60000 / beats_per_minute;
                    let click_time_ms = beat_time_ms / note_unit_clicks;
                    
                    // 초기 시간 갱신
                    self.last_update_time = Date::now();
                    
                    // 새 인터벌 설정
                    let link = ctx.link().clone();
                    let interval = Interval::new(click_time_ms as u32, move || {
                        link.send_message(MetronomeMsg::Tick);
                    });
                    
                    self.interval = Some(interval);
                    
                    // 오디오 컨텍스트가 없으면 생성
                    if self.sound_enabled && self.audio_ctx.is_none() {
                        match AudioContext::new() {
                            Ok(context) => {
                                self.audio_ctx = Some(context);
                            },
                            Err(err) => {
                                web_sys::console::error_1(&format!("오디오 컨텍스트 생성 실패: {:?}", err).into());
                            }
                        }
                    }
                }
                
                true
            },
            
            MetronomeMsg::SetTimeSignature(signature) => {
                // 박자 설정 업데이트
                self.time_signature = signature;
                
                // 비트 카운터 초기화
                self.current_beat = 0;
                
                // 재생 중인 경우 인터벌 재설정
                if self.is_playing {
                    // 기존 인터벌 제거
                    self.interval = None;
                    
                    // 새 타이머 인터벌 계산 (밀리초 단위)
                    let note_unit_clicks = self.note_unit.clicks_per_beat() as u32;
                    let beats_per_minute = self.bpm;
                    let beat_time_ms = 60000 / beats_per_minute;
                    let click_time_ms = beat_time_ms / note_unit_clicks;
                    
                    // 초기 시간 갱신
                    self.last_update_time = Date::now();
                    
                    // 새 인터벌 설정
                    let link = ctx.link().clone();
                    let interval = Interval::new(click_time_ms as u32, move || {
                        link.send_message(MetronomeMsg::Tick);
                    });
                    
                    self.interval = Some(interval);
                }
                
                true
            },
            
            MetronomeMsg::SetNoteUnit(unit) => {
                // 음표 단위 업데이트
                self.note_unit = unit;
                
                // 클릭 카운터 초기화
                self.current_click = 0;
                
                // 재생 중인 경우 인터벌 재설정
                if self.is_playing {
                    // 기존 인터벌 제거
                    self.interval = None;
                    
                    // 새 타이머 인터벌 계산 (밀리초 단위)
                    let note_unit_clicks = self.note_unit.clicks_per_beat() as u32;
                    let beats_per_minute = self.bpm;
                    let beat_time_ms = 60000 / beats_per_minute;
                    let click_time_ms = beat_time_ms / note_unit_clicks;
                    
                    // 초기 시간 갱신
                    self.last_update_time = Date::now();
                    
                    // 새 인터벌 설정
                    let link = ctx.link().clone();
                    let interval = Interval::new(click_time_ms as u32, move || {
                        link.send_message(MetronomeMsg::Tick);
                    });
                    
                    self.interval = Some(interval);
                }

                // UI 즉시 업데이트
                self.draw_metronome();
                
                true
            },
            
            MetronomeMsg::Tick => {
                if !self.is_playing {
                    return false;
                }
                
                let beats_per_measure = self.time_signature.beats_per_measure() as u32;
                let clicks_per_beat = self.note_unit.clicks_per_beat() as u32;
                
                // 클릭 및 박자 업데이트
                if self.current_click >= clicks_per_beat - 1 {
                    self.current_click = 0;
                    self.current_beat = (self.current_beat + 1) % beats_per_measure;
                } else {
                    self.current_click += 1;
                }
                
                // 총 클릭 수 증가 (애니메이션용)
                self.total_clicks += 1;
                
                // 소리 재생
                if self.sound_enabled {
                    let is_primary_beat = self.current_beat == 0 && self.current_click == 0;
                    self.play_click(is_primary_beat);
                }
                
                true
            },
            
            MetronomeMsg::ToggleSound => {
                self.sound_enabled = !self.sound_enabled;
                
                if !self.sound_enabled {
                    // 소리 비활성화 시 오디오 컨텍스트 중지
                    if let Some(context) = &self.audio_ctx {
                        let _ = context.suspend();
                    }
                } else if self.is_playing {
                    // 소리 활성화 및 재생 중이면 오디오 컨텍스트 재개
                    if let Some(context) = &self.audio_ctx {
                        let _ = context.resume();
                    } else {
                        // 없으면 새로 생성
                        match AudioContext::new() {
                            Ok(context) => {
                                self.audio_ctx = Some(context);
                            },
                            Err(err) => {
                                web_sys::console::error_1(&format!("오디오 컨텍스트 생성 실패: {:?}", err).into());
                            }
                        }
                    }
                }
                
                true
            },
            
            MetronomeMsg::UpdateCanvas => {
                self.draw_metronome();
                false
            },
            
            MetronomeMsg::TapTempo => {
                let now = Date::now();
                
                // 3초 이상 차이가 나면 탭 초기화
                if !self.tap_times.is_empty() && now - self.tap_times[self.tap_times.len() - 1] > 3000.0 {
                    self.tap_times.clear();
                }
                
                // 탭 시간 기록
                self.tap_times.push(now);
                
                // 최대 5개의 탭만 기록
                if self.tap_times.len() > 5 {
                    self.tap_times.remove(0);
                }
                
                // 최소 2개의 탭이 있어야 BPM 계산 가능
                if self.tap_times.len() >= 2 {
                    let mut intervals = Vec::new();
                    
                    // 각 탭 간의 간격 계산
                    for i in 1..self.tap_times.len() {
                        let interval = self.tap_times[i] - self.tap_times[i - 1];
                        intervals.push(interval);
                    }
                    
                    // 평균 간격 계산
                    let avg_interval = intervals.iter().sum::<f64>() / intervals.len() as f64;
                    
                    // BPM 계산 (밀리초 -> 분)
                    let new_bpm = (60000.0 / avg_interval).round() as u32;
                    
                    // 허용 범위(30-300) 내에 있는 경우만 적용
                    if new_bpm >= 30 && new_bpm <= 300 {
                        self.bpm = new_bpm;
                        
                        // 재생 중인 경우 인터벌 재설정
                        if self.is_playing {
                            // 기존 인터벌 제거
                            self.interval = None;
                            
                            // 새 타이머 인터벌 계산 (밀리초 단위)
                            let note_unit_clicks = self.note_unit.clicks_per_beat() as u32;
                            let beats_per_minute = self.bpm;
                            let beat_time_ms = 60000 / beats_per_minute;
                            let click_time_ms = beat_time_ms / note_unit_clicks;
                            
                            // 초기 시간 갱신
                            self.last_update_time = Date::now();
                            
                            // 새 인터벌 설정
                            let link = ctx.link().clone();
                            let interval = Interval::new(click_time_ms as u32, move || {
                                link.send_message(MetronomeMsg::Tick);
                            });
                            
                            self.interval = Some(interval);
                        }
                        
                        return true;
                    }
                }
                
                false
            },
            
            MetronomeMsg::ToggleAccent => {
                self.accent_enabled = !self.accent_enabled;
                
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        // 현재 값을 캡처
        let current_bpm = self.bpm;
        let is_playing = self.is_playing;
        let sound_enabled = self.sound_enabled;
        let time_signature = self.time_signature;
        let note_unit = self.note_unit;
        
        html! {
            <div class="metronome-container">
                <div class="metronome-compact-layout">
                    <div class="metronome-controls-top">
                        <div class="bpm-control-compact" style="display: flex; align-items: center; gap: 5px;">
                            <div class="bpm-buttons" style="display: flex; flex-direction: row; gap: 2px;">
                                <button 
                                    class="metronome-bpm-btn dec-large"
                                    style="padding: 2px 5px; font-size: 0.8rem;"
                                    onclick={ctx.link().callback(move |_| {
                                        let new_bpm = if current_bpm <= 35 { 30 } else { current_bpm - 5 };
                                        MetronomeMsg::SetBpm(new_bpm)
                                    })}
                                    disabled={current_bpm <= 30}
                                >
                                    {"- 5"}
                                </button>
                                
                                <button 
                                    class="metronome-bpm-btn dec-small"
                                    style="padding: 2px 5px; font-size: 0.8rem;"
                                    onclick={ctx.link().callback(move |_| {
                                        let new_bpm = if current_bpm <= 30 { 30 } else { current_bpm - 1 };
                                        MetronomeMsg::SetBpm(new_bpm)
                                    })}
                                    disabled={current_bpm <= 30}
                                >
                                    {"-"}
                                </button>
                            </div>
                            
                            <div class="bpm-display-compact" style="flex-grow: 1; text-align: center;">
                                <input 
                                    type="number" 
                                    min="30" 
                                    max="300" 
                                    value={current_bpm.to_string()}
                                    class="bpm-value-input"
                                    style="width: 80%; text-align: center; font-size: 1.2rem; font-weight: bold;"
                                    readonly={true}
                                />
                                <span 
                                    class="bpm-label"
                                    style="cursor: pointer; user-select: none;"
                                    onclick={ctx.link().callback(|_| MetronomeMsg::TapTempo)}
                                >{"BPM"}</span>
                            </div>
                            
                            <div class="bpm-buttons" style="display: flex; flex-direction: row; gap: 2px;">
                                <button 
                                    class="metronome-bpm-btn inc-small"
                                    style="padding: 2px 5px; font-size: 0.8rem;"
                                    onclick={ctx.link().callback(move |_| {
                                        let new_bpm = if current_bpm >= 300 { 300 } else { current_bpm + 1 };
                                        MetronomeMsg::SetBpm(new_bpm)
                                    })}
                                    disabled={current_bpm >= 300}
                                >
                                    {"+"}
                                </button>
                                
                                <button 
                                    class="metronome-bpm-btn inc-large"
                                    style="padding: 2px 5px; font-size: 0.8rem;"
                                    onclick={ctx.link().callback(move |_| {
                                        let new_bpm = if current_bpm >= 295 { 300 } else { current_bpm + 5 };
                                        MetronomeMsg::SetBpm(new_bpm)
                                    })}
                                    disabled={current_bpm >= 300}
                                >
                                    {"+ 5"}
                                </button>
                            </div>
                        </div>
                    </div>

                    <div class="metronome-display-compact">
                        <canvas ref={self.canvas_ref.clone()} width="1000" height="80" style="width: 100%; height: auto;"></canvas>
                    </div>

                    <div class="metronome-controls-bottom">
                        <div class="metronome-settings-compact" style="display: grid; grid-template-columns: 1fr 0.8fr 1fr; align-items: center; gap: 5px;">
                            <div class="time-signature-controls">
                                <select style="width: 100%;" onchange={ctx.link().callback(|e: Event| {
                                    let select = e.target_dyn_into::<web_sys::HtmlSelectElement>();
                                    if let Some(select) = select {
                                        match select.value().as_str() {
                                            "4/4" => MetronomeMsg::SetTimeSignature(TimeSignature::FourFour),
                                            "3/4" => MetronomeMsg::SetTimeSignature(TimeSignature::ThreeFour),
                                            "2/4" => MetronomeMsg::SetTimeSignature(TimeSignature::TwoFour),
                                            "6/8" => MetronomeMsg::SetTimeSignature(TimeSignature::SixEight),
                                            "9/8" => MetronomeMsg::SetTimeSignature(TimeSignature::NineEight),
                                            "12/8" => MetronomeMsg::SetTimeSignature(TimeSignature::TwelveEight),
                                            _ => MetronomeMsg::SetTimeSignature(TimeSignature::FourFour),
                                        }
                                    } else {
                                        MetronomeMsg::SetTimeSignature(TimeSignature::FourFour)
                                    }
                                })}>
                                    <option value="4/4" selected={time_signature == TimeSignature::FourFour}>{"4/4"}</option>
                                    <option value="3/4" selected={time_signature == TimeSignature::ThreeFour}>{"3/4"}</option>
                                    <option value="2/4" selected={time_signature == TimeSignature::TwoFour}>{"2/4"}</option>
                                    <option value="6/8" selected={time_signature == TimeSignature::SixEight}>{"6/8"}</option>
                                    <option value="9/8" selected={time_signature == TimeSignature::NineEight}>{"9/8"}</option>
                                    <option value="12/8" selected={time_signature == TimeSignature::TwelveEight}>{"12/8"}</option>
                                </select>
                            </div>
                            
                            <div class="control-buttons" style="align-items: center; display: flex; justify-content: center; gap: 1px;">
                                <button 
                                    class={if is_playing { "play-btn stop" } else { "play-btn play" }}
                                    onclick={ctx.link().callback(move |_| if is_playing { MetronomeMsg::Stop } else { MetronomeMsg::Start })}
                                >
                                    {if is_playing { "■" } else { "▶" }}
                                </button>
                                
                                <button 
                                    class={if sound_enabled { "sound-toggle sound-on" } else { "sound-toggle sound-off" }}
                                    onclick={ctx.link().callback(|_| MetronomeMsg::ToggleSound)}
                                >
                                    {if sound_enabled { "🔊" } else { "🔇" }}
                                </button>

                                <button 
                                    class={if self.accent_enabled { "play-btn accent" } else { "play-btn no-accent" }}
                                    onclick={ctx.link().callback(|_| MetronomeMsg::ToggleAccent)}
                                >
                                    {if self.accent_enabled { 
                                        html! {
                                            <span style="display: inline-block; transform: rotate(90deg); translate: 1px;">{">"}</span>
                                        }
                                    } else { 
                                        html! {"="} 
                                    }}
                                </button>
                            </div>
                            
                            <div class="note-unit-controls">
                                <select style="width: 100%;" onchange={ctx.link().callback(|e: Event| {
                                    let select = e.target_dyn_into::<web_sys::HtmlSelectElement>();
                                    if let Some(select) = select {
                                        match select.value().as_str() {
                                            "quarter" => MetronomeMsg::SetNoteUnit(NoteUnit::Quarter),
                                            "eighth" => MetronomeMsg::SetNoteUnit(NoteUnit::Eighth),
                                            "triplet" => MetronomeMsg::SetNoteUnit(NoteUnit::Triplet),
                                            "sixteenth" => MetronomeMsg::SetNoteUnit(NoteUnit::Sixteenth),
                                            _ => MetronomeMsg::SetNoteUnit(NoteUnit::Quarter),
                                        }
                                    } else {
                                        MetronomeMsg::SetNoteUnit(NoteUnit::Quarter)
                                    }
                                })}>
                                    <option value="quarter" selected={note_unit == NoteUnit::Quarter}>{"Quarter Note (1/4)"}</option>
                                    <option value="eighth" selected={note_unit == NoteUnit::Eighth}>{"Eighth Note (1/8)"}</option>
                                    <option value="triplet" selected={note_unit == NoteUnit::Triplet}>{"Triplet (1/3)"}</option>
                                    <option value="sixteenth" selected={note_unit == NoteUnit::Sixteenth}>{"Sixteenth Note (1/16)"}</option>
                                </select>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            // 캔버스 초기화
            self.draw_metronome();
            
            // 윈도우 리사이즈 이벤트 리스너 설정
            let canvas_ref = self.canvas_ref.clone();
            let link = ctx.link().clone();
            
            let resize_callback = Closure::wrap(Box::new(move || {
                // 캔버스 크기 업데이트
                if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                    let parent = canvas.parent_element().unwrap();
                    let width = parent.client_width() as u32;
                    // 높이를 너비의 일정 비율로 설정 (약 1:4 비율)
                    let height = (width as f32 * 0.25).min(80.0) as u32;
                    canvas.set_width(width);
                    canvas.set_height(height);
                }
                
                link.send_message(MetronomeMsg::UpdateCanvas);
            }) as Box<dyn FnMut()>);
            
            // 리사이즈 이벤트 리스너 등록
            web_sys::window()
                .unwrap()
                .add_event_listener_with_callback("resize", resize_callback.as_ref().unchecked_ref())
                .unwrap();
            
            // 메모리 누수 방지를 위해 클로저 유지
            resize_callback.forget();
            
            // 초기 캔버스 크기 설정
            if let Some(canvas) = self.canvas_ref.cast::<HtmlCanvasElement>() {
                let parent = canvas.parent_element().unwrap();
                let width = parent.client_width() as u32;
                // 높이를 너비의 일정 비율로 설정 (약 1:4 비율)
                let height = (width as f32 * 0.25).min(80.0) as u32;
                canvas.set_width(width);
                canvas.set_height(height);
                
                // 캔버스 업데이트
                self.draw_metronome();
            }
        }
    }

    fn destroy(&mut self, _ctx: &Context<Self>) {
        // 컴포넌트 제거 시 타이머 및 오디오 리소스 정리
        self.interval = None;
        
        if let Some(context) = &self.audio_ctx {
            let _ = context.close();
            self.audio_ctx = None;
        }
    }
}

impl Metronome {
    // 메트로놈 시각화 그리기
    fn draw_metronome(&self) {
        if let Some(canvas) = self.canvas_ref.cast::<HtmlCanvasElement>() {
            let context = canvas
                .get_context("2d")
                .unwrap()
                .unwrap()
                .dyn_into::<web_sys::CanvasRenderingContext2d>()
                .unwrap();
            
            let width = canvas.width() as f64;
            let height = canvas.height() as f64;
            
            // 디바이스 픽셀 비율 가져오기 (고해상도 디스플레이 대응)
            let dpr = web_sys::window()
                .unwrap()
                .device_pixel_ratio();
            
            // 고해상도 디스플레이 대응을 위한 캔버스 크기 조정
            if dpr > 1.0 {
                canvas.set_width((width * dpr) as u32);
                canvas.set_height((height * dpr) as u32);
                
                // 캔버스 CSS 크기는 그대로 유지
                canvas.set_attribute("style", &format!("width: {}px; height: {}px;", width, height)).unwrap();
                
                // 컨텍스트 스케일 조정
                context.scale(dpr, dpr).unwrap();
            }
            
            // 배경 지우기
            context.clear_rect(0.0, 0.0, width, height);
            context.set_fill_style(&"#0f1419".into()); // 피치 플롯과 동일한 다크 퍼플 배경
            context.fill_rect(0.0, 0.0, width, height);
            
            // 현재 박자 정보
            let beats_per_measure = self.time_signature.beats_per_measure() as usize;
            let clicks_per_beat = self.note_unit.clicks_per_beat() as usize;
            let total_clicks_per_measure = beats_per_measure * clicks_per_beat;
            
            // 현재 클릭 위치 계산 (현재 박 * 클릭 단위 + 현재 클릭)
            let current_position = if self.is_playing {
                (self.current_beat as usize * clicks_per_beat) + self.current_click as usize
            } else {
                usize::MAX // 재생 중이 아니면 하이라이트 없음
            };
            
            // 박자 표시 그리기 (원으로 표시)
            let available_width = width - 40.0; // 여백 고려
            let total_dots = beats_per_measure * clicks_per_beat;
            
            // 각 원의 최대 크기 계산 (가로 공간 기준)
            let max_per_row = if total_dots > 16 { 16 } else { total_dots };
            let circle_radius = (available_width / (max_per_row as f64 * 2.5)).min(15.0);
            let circle_spacing = circle_radius * 0.7;
            
            // 총 너비 계산 (원 그리기용)
            let row_width = (max_per_row as f64) * (circle_radius * 2.0 + circle_spacing);
            let start_x = (width - row_width) / 2.0 + circle_radius;
            let center_y = height / 2.0; // 정확한 중앙에 배치
            
            // 주 색상 정의 - 보라색 계열로 변경
            let primary_color = if self.is_playing {
                "#8b9aff" // 재생 중일 때는 밝은 보라색 (피치 플롯과 동일)
            } else {
                "#667eea" // 정지 상태일 때는 기본 보라색
            };
            let inactive_color = "#3a3f4e"; // 비활성 상태는 다크 퍼플 그리드 색상
            let first_beat_color = "#8b9aff"; // 첫 박자는 핫핑크 (피치 플롯과 동일)
            let dark_bg = "#2a2f3e"; // 테두리 색상도 보라색 계열로
            
            // 여러 행에 걸쳐 있을 경우 행간 간격 계산
            let rows_needed = (total_dots + max_per_row - 1) / max_per_row;
            let vertical_spacing = if rows_needed > 1 {
                // 원 사이즈의 2.2배 정도로 행간 설정 (여러 행일 때)
                circle_radius * 2.2
            } else {
                // 단일 행이면 간격 없음
                0.0
            };
            
            // 시작 y 위치 계산 (중앙 정렬)
            let start_y = center_y - ((rows_needed as f64 - 1.0) * vertical_spacing / 2.0);
            
            for beat in 0..beats_per_measure {
                for click in 0..clicks_per_beat {
                    let position = beat * clicks_per_beat + click;
                    let is_first_beat = beat == 0 && click == 0;
                    let is_beat_start = click == 0;
                    let is_current = position == current_position;
                    
                    // 원 위치 계산 (총 max_per_row개까지만 한 줄에 표시)
                    let row = position / max_per_row;
                    let col = position % max_per_row;
                    let x = start_x + col as f64 * (circle_radius * 2.0 + circle_spacing);
                    let y = start_y + row as f64 * vertical_spacing;
                    
                    // 현재 위치 하이라이트 효과 (원 주변에 글로우)
                    if is_current {
                        // 외부 글로우 효과
                        context.begin_path();
                        context.arc(x, y, circle_radius + 4.0, 0.0, std::f64::consts::PI * 2.0).unwrap();
                        context.set_shadow_color(primary_color);
                        context.set_shadow_blur(10.0);
                        context.set_stroke_style(&primary_color.into());
                        context.set_line_width(2.0);
                        context.stroke();
                        context.set_shadow_blur(0.0); // 다음 그리기에 영향 없도록 초기화
                    }
                    
                    // 원 그리기
                    context.begin_path();
                    context.arc(x, y, circle_radius, 0.0, std::f64::consts::PI * 2.0).unwrap();
                    
                    // 색상 설정 (첫 박, 각 박의 시작, 현재 위치, 일반)
                    if is_current {
                        context.set_fill_style(&primary_color.into()); // 민트 그린 (하이라이트)
                    } else if is_first_beat {
                        context.set_fill_style(&first_beat_color.into()); // 첫 박도 민트색 (더 옅게)
                        context.set_global_alpha(0.7); // 투명도 적용
                    } else if is_beat_start {
                        context.set_fill_style(&inactive_color.into()); // 옅은 민트색 (각 박의 시작)
                        context.set_global_alpha(0.7); // 투명도 적용
                    } else {
                        context.set_fill_style(&inactive_color.into()); // 옅은 민트색 (일반)
                        context.set_global_alpha(0.4); // 더 투명하게
                    }
                    
                    context.fill();
                    context.set_global_alpha(1.0); // 투명도 초기화
                    
                    // 테두리 그리기
                    context.set_stroke_style(&dark_bg.into());
                    context.set_line_width(1.5);
                    context.stroke();
                }
            }
            
            // 애니메이션 효과 (재생 중일 때)
            if self.is_playing && current_position < total_clicks_per_measure {
                // 현재 위치의 원 찾기
                let position = current_position;
                let row = position / max_per_row;
                let col = position % max_per_row;
                let x = start_x + col as f64 * (circle_radius * 2.0 + circle_spacing);
                let y = start_y + row as f64 * vertical_spacing;
                
                // 현재 시간 기준 애니메이션 진행도 계산
                let now = Date::now();
                let time_diff = now - self.last_update_time;
                
                // 틱당 시간 (밀리초)
                let note_unit_clicks = self.note_unit.clicks_per_beat() as f64;
                let beats_per_minute = self.bpm as f64;
                let tick_time_ms = 60000.0 / beats_per_minute / note_unit_clicks;
                
                // 펄스 효과 (0~1 사이의 값)
                let pulse_progress = (time_diff / tick_time_ms).min(1.0);
                let pulse_radius = circle_radius * (1.0 + (1.0 - pulse_progress) * 0.5);
                
                // 펄스 효과 그리기
                context.begin_path();
                context.arc(x, y, pulse_radius, 0.0, std::f64::consts::PI * 2.0).unwrap();
                context.set_stroke_style(&primary_color.into());
                context.set_line_width(2.0 * (1.0 - pulse_progress));
                context.set_global_alpha(1.0 - pulse_progress);
                context.stroke();
                context.set_global_alpha(1.0);
            }
        }
    }
    
    // 클릭 소리 재생
    fn play_click(&self, is_primary: bool) {
        // 오디오 컨텍스트가 없으면 재생하지 않음
        if let Some(audio_ctx) = &self.audio_ctx {
            // 오실레이터 노드 생성
            if let Ok(oscillator) = audio_ctx.create_oscillator() {
                // 주 박자와 나머지 박자의 주파수 다르게 설정
                if is_primary && self.accent_enabled {
                    oscillator.frequency().set_value(1200.0); // 1200Hz (첫 박자용 더 높은 소리)
                } else {
                    oscillator.frequency().set_value(800.0);  // 800Hz (일반 박자용)
                }
                
                // 게인 노드 생성 (볼륨 제어)
                if let Ok(gain) = audio_ctx.create_gain() {
                    // 오실레이터를 게인 노드에 연결
                    oscillator.connect_with_audio_node(&gain).unwrap();
                    
                    // 게인 노드를 출력에 연결
                    gain.connect_with_audio_node(&audio_ctx.destination()).unwrap();
                    
                    // 볼륨 설정 (첫 박자는 조금 더 크게)
                    if is_primary && self.accent_enabled {
                        gain.gain().set_value(0.3); // 첫 박자는 더 크게
                    } else {
                        gain.gain().set_value(0.2); // 일반 박자는 약간 작게
                    }
                    
                    // 현재 시간 가져오기
                    let current_time = audio_ctx.current_time();
                    
                    // 소리 길이 설정 (첫 박자는 조금 더 길게)
                    let duration = if is_primary && self.accent_enabled {
                        0.05 // 첫 박자는 50ms로 길게
                    } else {
                        0.03 // 일반 박자는 30ms
                    };
                    
                    // 게인 엔벨로프 설정 (빠른 어택, 빠른 릴리즈)
                    gain.gain().set_value_at_time(0.0, current_time).unwrap();
                    gain.gain().linear_ramp_to_value_at_time(if is_primary && self.accent_enabled { 0.3 } else { 0.2 }, current_time + 0.005).unwrap();
                    gain.gain().exponential_ramp_to_value_at_time(0.001, current_time + duration).unwrap();
                    
                    // 오실레이터 시작 및 중지 스케줄링
                    oscillator.start().unwrap();
                    oscillator.stop_with_when(current_time + duration).unwrap();
                }
            }
        } else if self.sound_enabled && self.is_playing {
            // 오디오 컨텍스트가 없지만 소리가 활성화되어 있고 재생 중이라면
            // 오디오 컨텍스트 생성을 시도하는 대신 경고 메시지만 출력
            web_sys::console::warn_1(&"오디오 컨텍스트가 없어 소리를 재생할 수 없습니다.".into());
        }
    }
} 