use wasm_bindgen::prelude::*;
use web_sys::{AudioContext, OscillatorNode, GainNode};
use yew::prelude::*;
use std::collections::HashMap;
use gloo_timers::callback::Timeout;

// 옥타브를 포함한 음 이름을 표현하는 구조체
#[derive(Debug, Clone, PartialEq, Eq)]
struct Note {
    name: String,      // 음 이름 (C, C#, D, 등)
    octave: i32,      // 옥타브 (2, 3, 4, 등)
}

impl Note {
    fn new(name: &str, octave: i32) -> Self {
        Self {
            name: name.to_string(),
            octave,
        }
    }

    // 음 이름과 옥타브를 합친 문자열 반환 (예: "C4")
    fn full_name(&self) -> String {
        format!("{}{}", self.name, self.octave)
    }

    // 주파수 계산 (A4 = 440Hz 기준)
    fn frequency(&self) -> f32 {
        // 모든 음 이름을 반음 단위로 변환
        let semitones = match self.name.as_str() {
            "C" => 0,
            "C#" | "Db" => 1,
            "D" => 2,
            "D#" | "Eb" => 3,
            "E" => 4,
            "F" => 5,
            "F#" | "Gb" => 6,
            "G" => 7,
            "G#" | "Ab" => 8,
            "A" => 9,
            "A#" | "Bb" => 10,
            "B" => 11,
            _ => 0, // 기본값 C
        };

        // A4(라4)는 MIDI 노트 번호 69, 주파수 440Hz
        let a4 = 440.0;
        
        // 현재 옥타브와 음의 MIDI 노트 번호 계산
        // C4는 MIDI 노트 번호 60, A4는 69
        let midi_note = (self.octave + 1) * 12 + semitones;
        
        // A4로부터의 반음 차이 계산
        let semitones_from_a4 = midi_note - 69;
        
        // 주파수 계산: f = 440 * 2^(n/12), n은 A4로부터의 반음 차이
        a4 * 2.0_f32.powf(semitones_from_a4 as f32 / 12.0)
    }
}

// 음계 종류를 나타내는 열거형
#[derive(Debug, Clone, PartialEq)]
pub enum ScaleType {
    Major,          // 장조 (예: 도레미파솔라시도)
    NaturalMinor,   // 자연단음계
    HarmonicMinor,  // 화성단음계
    MelodicMinor,   // 가락단음계
    PentatonicMajor, // 5음 장조
    PentatonicMinor, // 5음 단조
    Blues,           // 블루스
    Chromatic,       // 반음계
    Custom,          // 사용자 정의 음계
}

// 재생 방향 열거형
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum PlayDirection {
    Ascending,      // 상행
    Descending,     // 하행
    Both,           // 상행 후 하행
    BothDescendingFirst,  // 하행 후 상행
}

// 현재 재생 상태 열거형
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum PlaybackState {
    Stopped,        // 정지
    Playing,        // 재생 중
    Paused,         // 일시 정지
}

// 스케일 생성기 메시지 열거형
pub enum ScaleGeneratorMsg {
    SetStartNote(String, i32),  // 시작 근음 설정 (음 이름, 옥타브)
    SetEndNote(String, i32),    // 종료 근음 설정 (음 이름, 옥타브)
    SetNoteInterval(u32),       // 음 간의 인터벌 시간 설정
    SetSetInterval(u32),        // 스케일 셋 간의 인터벌 시간 설정
    AddInterval,                // 스케일 셋에 음정 추가
    RemoveInterval(usize),      // 스케일 셋에서 음정 제거
    SetIntervalValue(usize, String), // 특정 위치의 음정 값 설정
    SetPlayDirection(PlayDirection),  // 재생 방향 설정
    TogglePlayback,             // 재생/정지 토글
    Play,                       // 재생 시작
    Stop,                       // 정지
    PlayNextNote,               // 다음 음 재생
    InitAudioContext,           // 오디오 컨텍스트 초기화
}

// 스케일 생성기 컴포넌트
pub struct ScaleGenerator {
    start_note: Note,           // 시작 근음
    end_note: Note,             // 종료 근음
    note_interval_ms: u32,      // 음 간의 인터벌 시간 (밀리초)
    set_interval_ms: u32,       // 스케일 셋 간의 인터벌 시간 (밀리초)
    intervals: Vec<String>,     // 스케일 셋의 음정 목록
    play_direction: PlayDirection, // 재생 방향
    playback_state: PlaybackState, // 현재 재생 상태
    current_note_idx: usize,    // 현재 재생 중인 음 인덱스
    current_root_note: Option<Note>, // 현재 재생 중인 근음
    current_playing_note: Option<Note>, // 현재 재생 중인 음
    audio_ctx: Option<AudioContext>, // 오디오 컨텍스트
    notes_to_play: Vec<Note>,   // 재생할 음 목록
    play_timeout: Option<Timeout>, // 재생 타이머
    is_ascending: bool,         // 현재 상행 중인지 여부
}

impl Component for ScaleGenerator {
    type Message = ScaleGeneratorMsg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            start_note: Note::new("C", 4),  // 기본값 C4
            end_note: Note::new("C", 5),    // 기본값 C5
            note_interval_ms: 500,         // 기본값 500ms
            set_interval_ms: 1000,         // 기본값 1000ms
            intervals: vec!["1".to_string()], // 기본값 근음(1도)
            play_direction: PlayDirection::Ascending, // 기본값 상행
            playback_state: PlaybackState::Stopped, // 기본값 정지
            current_note_idx: 0,
            current_root_note: None,
            current_playing_note: None,
            audio_ctx: None,
            notes_to_play: Vec::new(),
            play_timeout: None,
            is_ascending: true,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            ScaleGeneratorMsg::SetStartNote(name, octave) => {
                self.start_note = Note::new(&name, octave);
                true
            }
            ScaleGeneratorMsg::SetEndNote(name, octave) => {
                self.end_note = Note::new(&name, octave);
                true
            }
            ScaleGeneratorMsg::SetNoteInterval(ms) => {
                // 최소값과 최대값 범위 지정
                if ms >= 100 && ms <= 1000 {
                    self.note_interval_ms = ms;
                    true
                } else {
                    false
                }
            }
            ScaleGeneratorMsg::SetSetInterval(ms) => {
                // 최소값과 최대값 범위 지정
                if ms >= 100 && ms <= 2000 {
                    self.set_interval_ms = ms;
                    true
                } else {
                    false
                }
            }
            ScaleGeneratorMsg::AddInterval => {
                // 기본값 "1"(근음)으로 새 인터벌 추가
                self.intervals.push("1".to_string());
                true
            }
            ScaleGeneratorMsg::RemoveInterval(index) => {
                // 최소 1개의 인터벌은 남겨둬야 함
                if self.intervals.len() > 1 && index < self.intervals.len() {
                    self.intervals.remove(index);
                    true
                } else {
                    false
                }
            }
            ScaleGeneratorMsg::SetIntervalValue(index, value) => {
                if index < self.intervals.len() {
                    self.intervals[index] = value;
                    true
                } else {
                    false
                }
            }
            ScaleGeneratorMsg::SetPlayDirection(direction) => {
                self.play_direction = direction;
                true
            }
            ScaleGeneratorMsg::Play => {
                // 이미 재생 중이면 무시
                if self.playback_state == PlaybackState::Playing {
                    return false;
                }
                
                // 오디오 컨텍스트 초기화 여부 확인
                if self.audio_ctx.is_none() {
                    // 오디오 컨텍스트 초기화
                    match AudioContext::new() {
                        Ok(ctx) => {
                            web_sys::console::log_1(&"오디오 컨텍스트 초기화 성공".into());
                            self.audio_ctx = Some(ctx);
                        }
                        Err(err) => {
                            web_sys::console::error_1(&format!("오디오 컨텍스트 초기화 실패: {:?}", err).into());
                            return false;
                        }
                    }
                }
                
                // 상태 업데이트
                self.playback_state = PlaybackState::Playing;
                
                // 재생할 노트 목록 생성
                self.generate_notes_to_play();
                
                // 첫 번째 노트 재생 준비
                self.current_note_idx = 0;
                if !self.notes_to_play.is_empty() {
                    // 현재 근음 설정 (첫 번째 노트)
                    self.current_root_note = Some(self.notes_to_play[0].clone());
                    
                    // 첫 노트 재생
                    ctx.link().send_message(ScaleGeneratorMsg::PlayNextNote);
                } else {
                    // 재생할 노트가 없으면 재생 중지
                    self.playback_state = PlaybackState::Stopped;
                }
                
                true
            }
            ScaleGeneratorMsg::Stop => {
                // 이미 정지 상태면 무시
                if self.playback_state == PlaybackState::Stopped {
                    return false;
                }
                
                // 타이머 중지
                self.play_timeout = None;
                
                // 상태 업데이트
                self.playback_state = PlaybackState::Stopped;
                self.current_note_idx = 0;
                self.current_root_note = None;
                self.current_playing_note = None;
                
                true
            }
            ScaleGeneratorMsg::TogglePlayback => {
                match self.playback_state {
                    PlaybackState::Playing => ctx.link().send_message(ScaleGeneratorMsg::Stop),
                    _ => ctx.link().send_message(ScaleGeneratorMsg::Play),
                }
                false
            }
            ScaleGeneratorMsg::PlayNextNote => {
                if self.playback_state != PlaybackState::Playing {
                    return false;
                }
                
                if let Some(audio_ctx) = &self.audio_ctx {
                    // 현재 재생 중인 노트가 있는지 확인
                    if self.current_note_idx < self.notes_to_play.len() {
                        // 현재 인덱스의 노트 가져오기
                        let current_note = self.notes_to_play[self.current_note_idx].clone();
                        
                        // SET_INTERVAL 노트인지 확인 (스케일 셋 구분자)
                        let is_set_interval = current_note.name == "SET_INTERVAL" && current_note.octave == -1;
                        
                        if !is_set_interval {
                            // 일반 노트인 경우, 현재 노트 표시 및 재생
                            self.current_playing_note = Some(current_note.clone());
                            
                            // 스케일 셋의 첫 번째 노트인 경우, 현재 근음 업데이트
                            if self.current_note_idx == 0 || 
                               (self.current_note_idx > 0 && 
                                self.notes_to_play[self.current_note_idx - 1].name == "SET_INTERVAL" && 
                                self.notes_to_play[self.current_note_idx - 1].octave == -1) {
                                self.current_root_note = Some(current_note.clone());
                            }
                            
                            self.play_note(&current_note);
                        }
                        
                        // 다음 노트 인덱스로 이동
                        self.current_note_idx += 1;
                        
                        // 다음 노트가 있으면 타이머 설정
                        if self.current_note_idx < self.notes_to_play.len() {
                            let link = ctx.link().clone();
                            
                            // 스케일 셋 인터벌인 경우 다른 타이머 시간 사용
                            let timeout_ms = if is_set_interval {
                                self.set_interval_ms
                            } else {
                                self.note_interval_ms
                            };
                            
                            let timeout = Timeout::new(timeout_ms, move || {
                                link.send_message(ScaleGeneratorMsg::PlayNextNote);
                            });
                            self.play_timeout = Some(timeout);
                        } else {
                            // 마지막 노트까지 재생 완료
                            self.playback_state = PlaybackState::Stopped;
                            self.current_note_idx = 0;
                        }
                    } else {
                        // 재생할 노트가 없는 경우
                        self.playback_state = PlaybackState::Stopped;
                        self.current_note_idx = 0;
                        self.current_playing_note = None;
                    }
                }
                
                true
            }
            ScaleGeneratorMsg::InitAudioContext => {
                if self.audio_ctx.is_none() {
                    match AudioContext::new() {
                        Ok(ctx) => {
                            web_sys::console::log_1(&"오디오 컨텍스트 초기화 성공".into());
                            self.audio_ctx = Some(ctx);
                        }
                        Err(err) => {
                            web_sys::console::error_1(&format!("오디오 컨텍스트 초기화 실패: {:?}", err).into());
                        }
                    }
                }
                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let notes = vec![
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"
        ];
        
        let octaves = vec![0, 1, 2, 3, 4, 5, 6, 7, 8];
        
        // 음정 옵션 목록
        let interval_options = vec![
            ("1", "1도 (근음)"),
            ("b2", "♭2도"),
            ("2", "2도"),
            ("b3", "♭3도"),
            ("3", "3도"),
            ("4", "4도"),
            ("b5", "♭5도"),
            ("5", "5도"),
            ("#5", "#5도"),
            ("6", "6도"),
            ("b7", "♭7도"),
            ("7", "7도"),
            ("8", "8도 (옥타브)"),
            ("b9", "♭9도"),
            ("9", "9도"),
            ("#9", "#9도"),
            ("b10", "♭10도"),
            ("10", "10도"),
            ("11", "11도"),
            ("#11", "#11도"),
            ("b12", "♭12도"),
            ("12", "12도"),
            ("#12", "#12도"),
            ("b13", "♭13도"),
            ("13", "13도"),
            ("b14", "♭14도"),
            ("14", "14도"),
            ("15", "15도"),
        ];
        
        html! {
            <div class="scale-generator">
                <div class="generator-section">
                    <div class="note-settings">
                        <div class="setting-group">
                            <label>{"시작 근음:"}</label>
                            
                            <div class="note-selectors">
                                <select
                                    value={self.start_note.name.clone()}
                                    onchange={ctx.link().callback(|e: Event| {
                                        let select = e.target_dyn_into::<web_sys::HtmlSelectElement>().unwrap();
                                        let name = select.value();
                                        ScaleGeneratorMsg::SetStartNote(name, 0) // 임시로 0 설정, 아래에서 업데이트됨
                                    })}
                                >
                                    {
                                        notes.iter().map(|note| {
                                            html! {
                                                <option value={note.to_string()} selected={&self.start_note.name == note}>
                                                    {note}
                                                </option>
                                            }
                                        }).collect::<Html>()
                                    }
                                </select>
                                
                                <select
                                    value={self.start_note.octave.to_string()}
                                    onchange={
                                        let name = self.start_note.name.clone();
                                        ctx.link().callback(move |e: Event| {
                                            let select = e.target_dyn_into::<web_sys::HtmlSelectElement>().unwrap();
                                            let octave = select.value().parse::<i32>().unwrap_or(4);
                                            ScaleGeneratorMsg::SetStartNote(name.clone(), octave)
                                        })
                                    }
                                >
                                    {
                                        octaves.iter().map(|&octave| {
                                            html! {
                                                <option value={octave.to_string()} selected={self.start_note.octave == octave}>
                                                    {octave}
                                                </option>
                                            }
                                        }).collect::<Html>()
                                    }
                                </select>
                            </div>
                        </div>
                        
                        <div class="setting-group">
                            <label>{"종료 근음:"}</label>
                            
                            <div class="note-selectors">
                                <select
                                    value={self.end_note.name.clone()}
                                    onchange={
                                        let octave = self.end_note.octave;
                                        ctx.link().callback(move |e: Event| {
                                            let select = e.target_dyn_into::<web_sys::HtmlSelectElement>().unwrap();
                                            let name = select.value();
                                            ScaleGeneratorMsg::SetEndNote(name, octave)
                                        })
                                    }
                                >
                                    {
                                        notes.iter().map(|note| {
                                            html! {
                                                <option value={note.to_string()} selected={&self.end_note.name == note}>
                                                    {note}
                                                </option>
                                            }
                                        }).collect::<Html>()
                                    }
                                </select>
                                
                                <select
                                    value={self.end_note.octave.to_string()}
                                    onchange={
                                        let name = self.end_note.name.clone();
                                        ctx.link().callback(move |e: Event| {
                                            let select = e.target_dyn_into::<web_sys::HtmlSelectElement>().unwrap();
                                            let octave = select.value().parse::<i32>().unwrap_or(5);
                                            ScaleGeneratorMsg::SetEndNote(name.clone(), octave)
                                        })
                                    }
                                >
                                    {
                                        octaves.iter().map(|&octave| {
                                            html! {
                                                <option value={octave.to_string()} selected={self.end_note.octave == octave}>
                                                    {octave}
                                                </option>
                                            }
                                        }).collect::<Html>()
                                    }
                                </select>
                            </div>
                        </div>
                    </div>
                </div>
                
                <div class="generator-section">
                    <div class="interval-settings">
                        <div class="setting-group">
                            <label>{"음 간의 인터벌 시간 (ms):"}</label>
                            <input 
                                type="range" 
                                min="100" 
                                max="1000" 
                                step="50" 
                                value={self.note_interval_ms.to_string()}
                                onchange={ctx.link().callback(|e: Event| {
                                    let input = e.target_dyn_into::<web_sys::HtmlInputElement>().unwrap();
                                    let value = input.value().parse::<u32>().unwrap_or(500);
                                    ScaleGeneratorMsg::SetNoteInterval(value)
                                })}
                            />
                            <span class="interval-value">{format!("{}ms", self.note_interval_ms)}</span>
                        </div>
                        
                        <div class="setting-group">
                            <label>{"스케일 셋 간의 인터벌 시간 (ms):"}</label>
                            <input 
                                type="range" 
                                min="100" 
                                max="2000" 
                                step="100" 
                                value={self.set_interval_ms.to_string()}
                                onchange={ctx.link().callback(|e: Event| {
                                    let input = e.target_dyn_into::<web_sys::HtmlInputElement>().unwrap();
                                    let value = input.value().parse::<u32>().unwrap_or(1000);
                                    ScaleGeneratorMsg::SetSetInterval(value)
                                })}
                            />
                            <span class="interval-value">{format!("{}ms", self.set_interval_ms)}</span>
                        </div>
                    </div>
                </div>
                
                <div class="generator-section">
                    <div class="scale-intervals-container">
                        <div class="scale-intervals-scroll">
                            {
                                self.intervals.iter().enumerate().map(|(idx, interval)| {
                                    html! {
                                        <div class="interval-item">
                                            <select
                                                value={interval.clone()}
                                                disabled={idx == 0}
                                                onchange={ctx.link().callback(move |e: Event| {
                                                    let select = e.target_dyn_into::<web_sys::HtmlSelectElement>().unwrap();
                                                    let value = select.value();
                                                    ScaleGeneratorMsg::SetIntervalValue(idx, value)
                                                })}
                                            >
                                                {
                                                    interval_options.iter().map(|(value, label)| {
                                                        html! {
                                                            <option value={value.to_string()} selected={interval == *value}>
                                                                {label}
                                                            </option>
                                                        }
                                                    }).collect::<Html>()
                                                }
                                            </select>
                                            
                                            {
                                                if idx > 0 {
                                                    html! {
                                                        <button
                                                            class="remove-interval"
                                                            onclick={ctx.link().callback(move |_| ScaleGeneratorMsg::RemoveInterval(idx))}
                                                        >
                                                            {"-"}
                                                        </button>
                                                    }
                                                } else {
                                                    html! {}
                                                }
                                            }
                                        </div>
                                    }
                                }).collect::<Html>()
                            }
                        </div>
                        
                        <button
                            class="add-interval"
                            onclick={ctx.link().callback(|_| ScaleGeneratorMsg::AddInterval)}
                        >
                            {"+"}
                        </button>
                    </div>
                </div>
                
                <div class="generator-section">
                    <div class="direction-settings">
                        <div class="radio-group">
                            <label>
                                <input 
                                    type="radio" 
                                    name="play-direction" 
                                    checked={self.play_direction == PlayDirection::Ascending}
                                    onchange={ctx.link().callback(|_| ScaleGeneratorMsg::SetPlayDirection(PlayDirection::Ascending))}
                                />
                                {"상행만"}
                            </label>
                            
                            <label>
                                <input 
                                    type="radio" 
                                    name="play-direction" 
                                    checked={self.play_direction == PlayDirection::Both}
                                    onchange={ctx.link().callback(|_| ScaleGeneratorMsg::SetPlayDirection(PlayDirection::Both))}
                                />
                                {"상행/하행"}
                            </label>
                            
                            <label>
                                <input 
                                    type="radio" 
                                    name="play-direction" 
                                    checked={self.play_direction == PlayDirection::BothDescendingFirst}
                                    onchange={ctx.link().callback(|_| ScaleGeneratorMsg::SetPlayDirection(PlayDirection::BothDescendingFirst))}
                                />
                                {"하행/상행"}
                            </label>
                            
                            <label>
                                <input 
                                    type="radio" 
                                    name="play-direction" 
                                    checked={self.play_direction == PlayDirection::Descending}
                                    onchange={ctx.link().callback(|_| ScaleGeneratorMsg::SetPlayDirection(PlayDirection::Descending))}
                                />
                                {"하행만"}
                            </label>
                        </div>
                    </div>
                </div>
                
                <div class="current-note-display">
                    <p>
                        {
                            if let Some(root_note) = &self.current_root_note {
                                format!("현재 근음: {}", root_note.full_name())
                            } else {
                                "현재 근음: -".to_string()
                            }
                        }
                    </p>
                    <p>
                        {
                            if let Some(playing_note) = &self.current_playing_note {
                                format!("현재 재생 음: {}", playing_note.full_name())
                            } else {
                                "현재 재생 음: -".to_string()
                            }
                        }
                    </p>
                </div>
                
                <div class="button-group">
                    <button
                        class="play-button"
                        onclick={ctx.link().callback(|_| ScaleGeneratorMsg::TogglePlayback)}
                    >
                        {
                            if self.playback_state == PlaybackState::Playing {
                                "■ 정지"
                            } else {
                                "▶ 재생"
                            }
                        }
                    </button>
                </div>
            </div>
        }
    }
}

impl ScaleGenerator {
    // 재생할 노트 목록 생성
    fn generate_notes_to_play(&mut self) {
        self.notes_to_play.clear();
        
        // 시작 근음부터 종료 근음까지의 모든 크로매틱 노트 생성
        let mut chromatic_notes = Vec::new();
        
        // 시작 근음과 종료 근음의 MIDI 노트 번호 계산
        let start_midi = (self.start_note.octave + 1) * 12 + 
            self.semitones_from_c(&self.start_note.name);
        let end_midi = (self.end_note.octave + 1) * 12 + 
            self.semitones_from_c(&self.end_note.name);
        
        // 시작 근음이 종료 근음보다 높을 경우, 방향 반전
        let (start_midi, end_midi) = if start_midi > end_midi {
            (end_midi, start_midi)
        } else {
            (start_midi, end_midi)
        };
        
        // 크로매틱 노트 목록 생성
        for midi in start_midi..=end_midi {
            let octave = midi / 12 - 1;
            let note_idx = midi % 12;
            let note_name = match note_idx {
                0 => "C",
                1 => "C#",
                2 => "D",
                3 => "D#",
                4 => "E",
                5 => "F",
                6 => "F#",
                7 => "G",
                8 => "G#",
                9 => "A",
                10 => "A#",
                11 => "B",
                _ => unreachable!(),
            };
            
            chromatic_notes.push(Note::new(note_name, octave));
        }
        
        // 재생 방향에 따라 노트 생성
        match self.play_direction {
            PlayDirection::Ascending => {
                // 상행
                self.generate_scale_for_range(&chromatic_notes);
            }
            PlayDirection::Descending => {
                // 하행
                let mut reversed = chromatic_notes;
                reversed.reverse();
                self.generate_scale_for_range(&reversed);
            }
            PlayDirection::Both => {
                // 상행 후 하행
                self.generate_scale_for_range(&chromatic_notes);
                
                // 상행과 하행 사이에 SET_INTERVAL 표시를 추가하여 
                // 스케일 셋 간 인터벌이 적용되도록 함
                self.notes_to_play.push(Note::new("SET_INTERVAL", -1));
                
                // 종료 근음에서 한 번 더 재생하지 않도록 중복 제거
                let mut reversed = chromatic_notes;
                reversed.reverse();
                reversed.remove(0); // 첫 번째 노트(종료 근음) 제거
                
                self.generate_scale_for_range(&reversed);
            }
            PlayDirection::BothDescendingFirst => {
                // 하행 후 상행
                let mut reversed = chromatic_notes.clone();
                reversed.reverse();
                self.generate_scale_for_range(&reversed);
                
                // 하행과 상행 사이에 SET_INTERVAL 표시를 추가
                self.notes_to_play.push(Note::new("SET_INTERVAL", -1));
                
                // 시작 근음에서 한 번 더 재생하지 않도록 중복 제거
                let mut ascending = chromatic_notes;
                ascending.remove(0); // 첫 번째 노트(시작 근음) 제거
                
                self.generate_scale_for_range(&ascending);
            }
        }
    }
    
    // 노트 범위에 대해 스케일 생성
    fn generate_scale_for_range(&mut self, notes: &[Note]) {
        if notes.is_empty() {
            return;
        }
        
        // 모든 근음에 대해 스케일 생성
        for (idx, root_note) in notes.iter().enumerate() {
            let mut scale_notes = Vec::new();
            
            // // 항상 근음을 첫 번째로 추가
            // scale_notes.push(root_note.clone());
            
            // 선택된 모든 인터벌에 대해 음 추가
            for interval in &self.intervals {
                // // 근음(1도)은 이미 추가했으므로 건너뜀
                // if interval == "1" {
                //     continue;
                // }
                
                // 인터벌에 따른 음 계산 및 추가
                if let Some(note) = self.compute_note_from_interval(root_note, interval) {
                    scale_notes.push(note);
                }
            }
            
            // 현재 notes_to_play에 추가
            self.notes_to_play.append(&mut scale_notes);
            
            // 마지막 근음이 아니라면 근음 사이 간격을 위한 표시를 추가
            // -1을 옥타브로 갖는 특별한 Note를 추가하여 이 노트가 나왔을 때 
            // set_interval_ms 만큼 대기하도록 함
            if idx < notes.len() - 1 {
                self.notes_to_play.push(Note::new("SET_INTERVAL", -1));
            }
        }
    }
    
    // 지정된 노트 재생
    fn play_note(&self, note: &Note) {
        if let Some(audio_ctx) = &self.audio_ctx {
            if let Ok(oscillator) = audio_ctx.create_oscillator() {
                // 주파수 설정
                oscillator.frequency().set_value(note.frequency());
                
                // 게인 노드 생성 (볼륨 제어)
                if let Ok(gain) = audio_ctx.create_gain() {
                    // 오실레이터를 게인 노드에 연결
                    if oscillator.connect_with_audio_node(&gain).is_err() {
                        web_sys::console::error_1(&"오실레이터 연결 실패".into());
                        return;
                    }
                    
                    // 게인 노드를 출력에 연결
                    if gain.connect_with_audio_node(&audio_ctx.destination()).is_err() {
                        web_sys::console::error_1(&"게인 노드 연결 실패".into());
                        return;
                    }
                    
                    // 볼륨 설정
                    gain.gain().set_value(0.3); // 30% 볼륨
                    
                    // 현재 시간 가져오기
                    let current_time = audio_ctx.current_time();
                    
                    // 게인 엔벨로프 설정 (빠른 어택, 빠른 릴리즈)
                    let _ = gain.gain().set_value_at_time(0.0, current_time);
                    let _ = gain.gain().linear_ramp_to_value_at_time(0.3, current_time + 0.01);
                    let _ = gain.gain().exponential_ramp_to_value_at_time(0.001, current_time + 0.3);
                    
                    // 오실레이터 시작 및 중지 스케줄링
                    let _ = oscillator.start();
                    let _ = oscillator.stop_with_when(current_time + 0.4);
                    
                    // 로그 출력
                    web_sys::console::log_1(&format!("노트 재생: {} ({:.2} Hz)",
                        note.full_name(), note.frequency()).into());
                }
            }
        }
    }
    
    // 음 이름의 C로부터의 반음 수 계산
    fn semitones_from_c(&self, note_name: &str) -> i32 {
        match note_name {
            "C" => 0,
            "C#" => 1,
            "Db" => 1,
            "D" => 2,
            "D#" => 3,
            "Eb" => 3,
            "E" => 4,
            "F" => 5,
            "F#" => 6,
            "Gb" => 6,
            "G" => 7,
            "G#" => 8,
            "Ab" => 8,
            "A" => 9,
            "A#" => 10,
            "Bb" => 10,
            "B" => 11,
            _ => 0,
        }
    }
    
    // 인터벌 문자열을 반음 개수로 변환
    fn interval_semitones(&self, interval: &str) -> i32 {
        match interval {
            "1" => 0,     // 근음 (완전1도)
            "b2" => 1,    // 단2도
            "2" => 2,     // 장2도
            "b3" => 3,    // 단3도
            "3" => 4,     // 장3도
            "4" => 5,     // 완전4도
            "b5" => 6,    // 감5도
            "5" => 7,     // 완전5도
            "#5" => 8,    // 증5도
            "6" => 9,     // 장6도
            "b7" => 10,   // 단7도
            "7" => 11,    // 장7도
            "8" => 12,    // 옥타브
            "b9" => 13,   // ♭9도 (2옥타브 단2도)
            "9" => 14,    // 9도 (2옥타브 장2도)
            "#9" => 15,   // #9도 (2옥타브 증2도)
            "b10" => 15,  // ♭10도 (2옥타브 단3도)
            "10" => 16,   // 10도 (2옥타브 장3도)
            "11" => 17,   // 11도 (2옥타브 완전4도)
            "#11" => 18,  // #11도 (2옥타브 증4도)
            "b12" => 18,  // ♭12도 (2옥타브 감5도)
            "12" => 19,   // 12도 (2옥타브 완전5도)
            "#12" => 20,  // #12도 (2옥타브 증5도)
            "b13" => 20,  // ♭13도 (2옥타브 단6도)
            "13" => 21,   // 13도 (2옥타브 장6도)
            "b14" => 22,  // ♭14도 (2옥타브 단7도)
            "14" => 23,   // 14도 (2옥타브 장7도)
            "15" => 24,   // 15도 (2옥타브)
            _ => 0,       // 기본값은 근음
        }
    }
    
    // 근음과 음정으로 새 노트 계산
    fn compute_note_from_interval(&self, root: &Note, interval: &str) -> Option<Note> {
        // 인터벌의 반음 수 계산
        let semitones = self.interval_semitones(interval);
        
        // 근음의 MIDI 노트 번호 계산
        let root_midi = (root.octave + 1) * 12 + self.semitones_from_c(&root.name);
        
        // 인터벌을 적용한 새 MIDI 노트 번호
        let new_midi = root_midi + semitones;
        
        // MIDI 노트 번호에서 옥타브와 음 이름 계산
        let octave = new_midi / 12 - 1;
        let note_idx = new_midi % 12;
        let note_name = match note_idx {
            0 => "C",
            1 => "C#",
            2 => "D",
            3 => "D#",
            4 => "E",
            5 => "F",
            6 => "F#",
            7 => "G",
            8 => "G#",
            9 => "A",
            10 => "A#",
            11 => "B",
            _ => return None,
        };
        
        Some(Note::new(note_name, octave))
    }
} 