use wasm_bindgen::prelude::*;
use web_sys::{AudioContext, AudioNode, AudioParam, GainNode, HtmlAudioElement};
use yew::prelude::*;
use std::collections::HashMap;
use std::rc::Rc;
use gloo_timers::callback::Timeout;
use wasm_bindgen::closure::Closure;
use web_sys::console;

// 피아노 키 정보를 위한 구조체
#[derive(Clone, PartialEq)]
struct PianoKey {
    name: String,    // 노트 이름 (C, C#, D 등)
    octave: i32,     // 옥타브 (0-8)
    is_black: bool,  // 검은 건반 여부
    is_pressed: bool, // 현재 눌려있는지 여부
}

impl PianoKey {
    fn new(name: &str, octave: i32, is_black: bool) -> Self {
        Self {
            name: name.to_string(),
            octave,
            is_black,
            is_pressed: false,
        }
    }

    // 완전한 노트 이름 반환 (예: C4, Eb5)
    fn full_name(&self) -> String {
        format!("{}{}", self.name, self.octave)
    }

    // 오디오 파일 경로 반환
    fn audio_path(&self) -> String {
        // 샵(#)을 플랫(b)으로 변환하여 파일 경로 생성
        let note_name = if self.name.contains("#") {
            match self.name.as_str() {
                "C#" => "Db",
                "D#" => "Eb",
                "F#" => "Gb",
                "G#" => "Ab",
                "A#" => "Bb",
                _ => &self.name
            }
        } else {
            &self.name
        };
        
        // 파일 이름 포맷: Piano.ff.노트옥타브.mp3 (예: Piano.ff.C4.mp3 또는 Piano.ff.Db4.mp3)
        format!("static/piano/Piano.ff.{}{}.mp3", note_name, self.octave)
    }
}

// 피아노 컴포넌트 메시지
pub enum PianoMsg {
    KeyPressed(usize),              // 키가 눌렸을 때
    KeyReleased(usize),             // 키가 떼어졌을 때
    ToggleSustain,                  // 서스테인 토글
    StopSound(String),              // 특정 소리 정지
    SetStartOctave(i32),            // 시작 옥타브 설정
    ScrollPiano(i32),               // 피아노 스크롤
}

// 피아노 컴포넌트
pub struct PianoKeyboard {
    keys: Vec<PianoKey>,            // 모든 피아노 키
    active_sounds: HashMap<String, HtmlAudioElement>, // 현재 재생 중인 소리
    sustain: bool,                  // 서스테인 상태
    start_octave: i32,              // 표시할 시작 옥타브
    audio_ctx: Option<AudioContext>, // 오디오 컨텍스트
}

impl Component for PianoKeyboard {
    type Message = PianoMsg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        // 모든 88개 키 생성 (A0-C8)
        let mut keys = Vec::new();
        let note_names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
        let black_keys = [false, true, false, true, false, false, true, false, true, false, true, false];
        
        // A0부터 C8까지 모든 키 생성
        for octave in 0..=8 {
            for i in 0..note_names.len() {
                let name = note_names[i];
                let is_black = black_keys[i];
                
                // A0가 첫 번째 키이므로 0 옥타브는 A부터 시작
                if octave == 0 && name < "A" {
                    continue;
                }
                
                // C8이 마지막 키이므로 8 옥타브는 C까지만
                if octave == 8 && name > "C" {
                    break;
                }
                
                keys.push(PianoKey::new(name, octave, is_black));
            }
        }

        Self {
            keys,
            active_sounds: HashMap::new(),
            sustain: false,
            start_octave: 2, // 기본 시작 옥타브는 2
            audio_ctx: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            PianoMsg::KeyPressed(index) => {
                if index < self.keys.len() {
                    self.keys[index].is_pressed = true;
                    
                    // 소리 재생
                    let audio = HtmlAudioElement::new_with_src(&self.keys[index].audio_path())
                        .expect("오디오 요소 생성 실패");
                    
                    let _ = audio.play().expect("오디오 재생 실패");
                    let key_name = self.keys[index].full_name();
                    self.active_sounds.insert(key_name, audio);
                    
                    true
                } else {
                    false
                }
            },
            PianoMsg::KeyReleased(index) => {
                if index < self.keys.len() {
                    self.keys[index].is_pressed = false;
                    
                    // 서스테인이 꺼져 있으면 1초 후에 소리 정지
                    if !self.sustain {
                        let key_name = self.keys[index].full_name();
                        
                        if let Some(_) = self.active_sounds.get(&key_name) {
                            let key_name_clone = key_name.clone();
                            let link = ctx.link().clone();
                            
                            // 1초 후에 소리 정지
                            let timeout = Timeout::new(500, move || {
                                link.send_message(PianoMsg::StopSound(key_name_clone));
                            });
                            
                            // 타임아웃이 가비지 컬렉션되지 않도록 함
                            timeout.forget();
                        }
                    }
                    
                    true
                } else {
                    false
                }
            },
            PianoMsg::ToggleSustain => {
                self.sustain = !self.sustain;
                
                // 서스테인이 꺼졌을 때 눌린 키가 없는 소리들 정지
                if !self.sustain {
                    let keys_to_stop: Vec<String> = self.active_sounds.keys()
                        .filter(|k| {
                            !self.keys.iter().any(|key| key.is_pressed && key.full_name() == **k)
                        })
                        .cloned()
                        .collect();
                    
                    for key_name in keys_to_stop {
                        let key_name_clone = key_name.clone();
                        let link = ctx.link().clone();
                        
                        // 1초 후에 소리 정지
                        let timeout = Timeout::new(1000, move || {
                            link.send_message(PianoMsg::StopSound(key_name_clone));
                        });
                        
                        // 타임아웃이 가비지 컬렉션되지 않도록 함
                        timeout.forget();
                    }
                }
                
                true
            },
            PianoMsg::StopSound(key_name) => {
                if let Some(audio) = self.active_sounds.get(&key_name) {
                    let _ = audio.pause();
                    let _ = audio.set_current_time(0.0);
                    self.active_sounds.remove(&key_name);
                }
                false
            },
            PianoMsg::SetStartOctave(octave) => {
                if octave >= 0 && octave <= 4 { // A0-C8 범위를 고려
                    self.start_octave = octave;
                    true
                } else {
                    false
                }
            },
            PianoMsg::ScrollPiano(delta) => {
                let new_octave = self.start_octave + delta;
                if new_octave >= 0 && new_octave <= 4 {
                    self.start_octave = new_octave;
                    true
                } else {
                    false
                }
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        // 현재 표시할 키 범위 계산 (3옥타브 + 다음 옥타브 C까지 표시)
        let end_octave = self.start_octave + 4;
        let visible_keys: Vec<(usize, &PianoKey)> = self.keys.iter().enumerate()
            .filter(|(_, key)| {
                // 시작 옥타브부터 3옥타브 범위 내에 있거나,
                // 다음 옥타브의 C 음까지만 포함
                (key.octave >= self.start_octave && key.octave < end_octave) || 
                (key.octave == end_octave && key.name == "C")
            })
            .collect();

        html! {
            <div class="piano-container">
                <div class="piano-layout">
                    <div class="piano-section">
                        <div class="keyboard-container">
                            <div class="piano-keyboard">
                                {
                                    // 하얀 건반 먼저 렌더링
                                    visible_keys.iter().filter(|(_, key)| !key.is_black).map(|(index, key)| {
                                        let i = *index;
                                        html! {
                                            <div 
                                                class={classes!("piano-key", "white-key", if key.is_pressed { "pressed" } else { "" })}
                                                onmousedown={ctx.link().callback(move |_| PianoMsg::KeyPressed(i))}
                                                onmouseup={ctx.link().callback(move |_| PianoMsg::KeyReleased(i))}
                                                onmouseleave={ctx.link().callback(move |_| PianoMsg::KeyReleased(i))}
                                                title={key.full_name()}
                                                style="flex: 1;"
                                            >
                                                <span class="key-label">{key.full_name()}</span>
                                            </div>
                                        }
                                    }).collect::<Html>()
                                }
                                {
                                    // 검은 건반 나중에 렌더링 (하얀 건반 위에 겹치게)
                                    visible_keys.iter().filter(|(_, key)| key.is_black).map(|(index, key)| {
                                        let i = *index;
                                        // 검은 건반의 위치 계산을 위한 흰 건반 인덱스 찾기
                                        let prev_white_key_idx = visible_keys.iter()
                                            .filter(|(_, k)| !k.is_black)
                                            .position(|(_, k)| {
                                                // 같은 옥타브내에서 현재 검은 건반 바로 앞에 있는 흰 건반 찾기
                                                // 예: C# 앞에는 C, D# 앞에는 D
                                                let note_name = &key.name[0..1]; // 첫 글자만 추출 (C#에서 C)
                                                k.octave == key.octave && k.name == note_name
                                            })
                                            .unwrap_or(0);
                                        
                                        // 흰 건반의 개수
                                        let white_keys_count = visible_keys.iter().filter(|(_, k)| !k.is_black).count();
                                        
                                        // 검은 건반 위치 계산: 해당 흰 건반과 다음 흰 건반 사이에 위치
                                        // 흰 건반 사이의 각 위치에 검은 건반을 배치
                                        let note_name = key.name.as_str();
                                        let offset = match note_name {
                                            "C#" => 1.0, // C#는 C 건반 위 약간 오른쪽에 위치
                                            "D#" => 1.0, // D#는 D 건반 위 약간 오른쪽에 위치
                                            "F#" => 1.0, // F#는 F 건반 위 약간 오른쪽에 위치
                                            "G#" => 1.0, // G#는 G 건반 위 약간 오른쪽에 위치
                                            "A#" => 1.0, // A#는 A 건반 위 약간 오른쪽에 위치
                                            _ => 0.5,    // 기본 값
                                        };
                                        
                                        // 각 흰 건반의 너비를 백분율로 계산 (border와 margin 고려)
                                        let white_key_width = 100.0 / (white_keys_count as f32);
                                        
                                        // 검은 건반 위치 계산: 이전 흰 건반 위치 + (흰 건반 너비 * offset)
                                        let position = (prev_white_key_idx as f32 * white_key_width) + (white_key_width * offset);
                                        
                                        html! {
                                            <div 
                                                class={classes!("piano-key", "black-key", if key.is_pressed { "pressed" } else { "" })}
                                                style={format!("top: 0; left: {}%", position)}
                                                onmousedown={ctx.link().callback(move |_| PianoMsg::KeyPressed(i))}
                                                onmouseup={ctx.link().callback(move |_| PianoMsg::KeyReleased(i))}
                                                onmouseleave={ctx.link().callback(move |_| PianoMsg::KeyReleased(i))}
                                                title={key.full_name()}
                                            >
                                                <span class="key-label">{key.full_name()}</span>
                                            </div>
                                        }
                                    }).collect::<Html>()
                                }
                            </div>
                        </div>
                    </div>
                    <div class="settings-section">
                        <div class="settings-container">
                            <div class="settings-row">
                                <div class="octave-control">
                                    <button 
                                        onclick={ctx.link().callback(|_| PianoMsg::ScrollPiano(-1))}
                                        disabled={self.start_octave == 0}
                                        title="옥타브 아래로"
                                    >
                                        {"◀"}
                                    </button>
                                    <div class="octave-display">{format!("옥타브: {}-{}", self.start_octave, end_octave)}</div>
                                    <button 
                                        onclick={ctx.link().callback(|_| PianoMsg::ScrollPiano(1))}
                                        disabled={self.start_octave >= 4}
                                        title="옥타브 위로"
                                    >
                                        {"▶"}
                                    </button>
                                </div>
                                <div class="sustain-control">
                                    <button 
                                        class={classes!("sustain-button", if self.sustain { "active" } else { "" })}
                                        onclick={ctx.link().callback(|_| PianoMsg::ToggleSustain)}
                                        title={if self.sustain { "서스테인 끄기" } else { "서스테인 켜기" }}
                                    >
                                        {"서스테인"}
                                    </button>
                                </div>
                            </div>
                            
                            <div class="settings-row future-features">
                                <div class="placeholder-text">{"추가 기능이 이곳에 배치됩니다"}</div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        }
    }
}

impl PianoKeyboard {
    // 검은 건반의 위치 계산
    fn get_black_key_position(&self, key: &PianoKey) -> String {
        // 각 검은 건반의 상대적 위치를 계산
        // 이 값은 CSS에서 위치를 조정하는 데 사용됨
        let note_name = key.name.as_str();
        
        // 12개 음계 중 검은 건반의 상대적 위치
        let position = match note_name {
            "C#" => 1,
            "D#" => 3,
            "F#" => 6,
            "G#" => 8, 
            "A#" => 10,
            _ => 0, // 기본값 (발생하지 않아야 함)
        };
        
        // 흰 건반 너비의 비율로 위치 계산
        format!("--black-key-position: {};", position)
    }
    
    // 건반에 해당하는 소리 재생
    fn play_sound(&mut self, ctx: &Context<Self>, key_idx: usize) {
        // 이미 재생 중인 소리가 있으면 중지
        if let Some(audio) = self.active_sounds.get(&self.keys[key_idx].full_name()) {
            let _ = audio.pause();
            let _ = audio.set_current_time(0.0);
        }
        
        let key = &self.keys[key_idx];
        let file_path = key.audio_path();
        
        // 문서 객체 모델에서 window 객체 가져오기
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                // 새 오디오 요소 생성
                if let Ok(element) = document.create_element("audio") {
                    let audio_element: HtmlAudioElement = element
                        .dyn_into::<HtmlAudioElement>()
                        .expect("HtmlAudioElement로 변환할 수 없습니다");
                    
                    // 피아노 음원 파일 경로 설정
                    audio_element.set_src(&file_path);
                    
                    // 볼륨 설정
                    audio_element.set_volume(0.7);
                    
                    // 오디오 요소 미리 로드
                    let _ = audio_element.load();
                    
                    // 시작 위치를 0초로 설정 후 재생
                    audio_element.set_current_time(0.0);
                    
                    // 오디오 재생
                    if let Err(err) = audio_element.play() {
                        console::error_1(&format!("오디오 재생 실패: {:?}", err).into());
                    } else {
                        console::log_1(&format!("피아노 노트 재생: {}", key.full_name()).into());
                        
                        // 재생 중인 소리 목록에 추가
                        self.active_sounds.insert(key.full_name(), audio_element);
                    }
                }
            }
        }
    }
    
    // 건반에 해당하는 소리 중지
    fn stop_sound(&mut self, key_idx: usize) {
        if let Some(audio) = self.active_sounds.remove(&self.keys[key_idx].full_name()) {
            let _ = audio.pause();
            let _ = audio.set_src("");  // 리소스 해제
            
            console::log_1(&format!("피아노 노트 중지: {}", self.keys[key_idx].full_name()).into());
        }
    }
}

// 피아노 함수 컴포넌트 (외부에서 사용하기 위한 함수형 래퍼)
#[function_component(Piano)]
pub fn piano_keyboard() -> Html {
    html! {
        <div style="aspect-ratio: calc(26.7/3.0); width: 100%; height: 100%; overflow: hidden;">
            <PianoKeyboard />
        </div>
    }
} 