use wasm_bindgen::prelude::*;
use web_sys::{AudioContext, OscillatorNode, GainNode, HtmlAudioElement, AudioNode};
use yew::prelude::*;
use std::collections::HashMap;
use gloo_timers::callback::Timeout;
use wasm_bindgen::closure::Closure;

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

    // 피아노 음원 파일 경로 반환
    fn piano_file_path(&self) -> String {
        // 음 이름을 파일명 형식으로 변환 (예: C# -> Db)
        let file_name = match self.name.as_str() {
            "C#" => "Db",
            "D#" => "Eb",
            "F#" => "Gb",
            "G#" => "Ab",
            "A#" => "Bb",
            name => name,
        };

        // 유효한 옥타브 범위 체크 (A0-C8)
        let octave = self.octave.max(0).min(8);
        
        // A0은 최저음, C8은 최고음
        if (octave == 0 && file_name < "A") || (octave == 8 && file_name > "C") {
            // 범위를 벗어나면 가장 가까운 음 사용
            if octave == 0 {
                return format!("/static/piano/Piano.ff.A0.mp3");
            } else {
                return format!("/static/piano/Piano.ff.C8.mp3");
            }
        }

        format!("/static/piano/Piano.ff.{}{}.mp3", file_name, octave)
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
    SetBpm(u32),                // BPM 설정
    AddInterval,                // 스케일 셋에 음정 추가
    RemoveInterval(usize),      // 스케일 셋에서 음정 제거
    SetIntervalValue(usize, String), // 특정 위치의 음정 값 설정
    SetPlayDirection(PlayDirection),  // 재생 방향 설정
    TogglePlayback,             // 재생/정지 토글
    Play,                       // 재생 시작
    Stop,                       // 정지
    PlayNextNote,               // 다음 음 재생
    InitAudioContext,           // 오디오 컨텍스트 초기화
    ClearIntervals,             // 인터벌 초기화 (근음만 남김)
}

// 스케일 생성기 컴포넌트
pub struct ScaleGenerator {
    start_note: Note,           // 시작 근음
    end_note: Note,             // 종료 근음
    bpm: u32,                   // BPM (Beats Per Minute)
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
    audio_element: Option<HtmlAudioElement>, // 오디오 요소
}

impl Component for ScaleGenerator {
    type Message = ScaleGeneratorMsg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            start_note: Note::new("C", 4),  // 기본값 C4
            end_note: Note::new("C", 5),    // 기본값 C5
            bpm: 120,                       // 기본값 120 BPM
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
            audio_element: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            ScaleGeneratorMsg::SetStartNote(name, octave) => {
                let mut adjusted_octave = octave;
                
                // 옥타브 범위 조정
                adjusted_octave = match name.as_str() {
                    "A" | "A#" | "B" => adjusted_octave.max(0).min(7),  // 0~7 범위로 제한
                    "C" => adjusted_octave.max(1).min(8),               // 1~8 범위로 제한
                    _ => adjusted_octave.max(1).min(7),                 // 1~7 범위로 제한
                };
                
                self.start_note = Note::new(&name, adjusted_octave);
                true
            }
            ScaleGeneratorMsg::SetEndNote(name, octave) => {
                let mut adjusted_octave = octave;
                
                // 옥타브 범위 조정
                adjusted_octave = match name.as_str() {
                    "A" | "A#" | "B" => adjusted_octave.max(0).min(7),  // 0~7 범위로 제한
                    "C" => adjusted_octave.max(1).min(8),               // 1~8 범위로 제한
                    _ => adjusted_octave.max(1).min(7),                 // 1~7 범위로 제한
                };
                
                self.end_note = Note::new(&name, adjusted_octave);
                true
            }
            ScaleGeneratorMsg::SetBpm(bpm) => {
                self.bpm = bpm;
                true
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
                
                // 현재 재생 중인 오디오 중지 및 리소스 해제
                if let Some(audio) = &self.audio_element {
                    let _ = audio.pause();
                    let _ = audio.set_src(""); // 리소스 해제
                    self.audio_element = None;
                }
                
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
                
                if self.current_note_idx < self.notes_to_play.len() {
                    // 현재 인덱스의 노트 가져오기
                    let current_note = self.notes_to_play[self.current_note_idx].clone();
                    
                    // SET_INTERVAL 노트인지 확인 (스케일 셋 구분자)
                    let is_set_interval = current_note.name == "SET_INTERVAL" && current_note.octave == -1;
                    
                    // 다음 노트 인덱스 계산
                    let next_idx = self.current_note_idx + 1;
                    
                    // BPM 기반 타이밍 계산 (밀리초 단위)
                    // BPM은 분당 박자 수, 60000ms / BPM = 한 박자당 밀리초
                    let beat_time_ms = 60000 / self.bpm;
                    
                    // 스케일 셋의 마지막 노트 여부 확인
                    let is_scale_set_end = next_idx < self.notes_to_play.len() && 
                                           self.notes_to_play[next_idx].name == "SET_INTERVAL" && 
                                           self.notes_to_play[next_idx].octave == -1;
                    
                    // 전체 스케일의 마지막 노트 여부 확인
                    let is_last_note = next_idx >= self.notes_to_play.len();
                    
                    // 기본 음표 지속시간은 한 박자(beat_time_ms)
                    let mut note_duration = beat_time_ms;
                    
                    // 마지막 노트 처리 (스케일 셋 마지막 또는 전체 마지막)
                    if is_scale_set_end || is_last_note {
                        note_duration = beat_time_ms * 4; // 마지막 노트는 4배 길게
                    }
                    
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
                        
                        // 피아노 음원으로 노트 재생
                        self.play_piano_note(ctx, &current_note);
                        
                        // 다음 노트를 위해 인덱스 증가
                        self.current_note_idx = next_idx;
                        
                        // 다음 노트를 위한 타이머 설정
                        if !is_last_note {
                            let link = ctx.link().clone();
                            let timeout = Timeout::new(note_duration, move || {
                                link.send_message(ScaleGeneratorMsg::PlayNextNote);
                            });
                            self.play_timeout = Some(timeout);
                        } else {
                            // 마지막 노트인 경우 정지 메시지 예약
                            let link = ctx.link().clone();
                            let timeout = Timeout::new(note_duration, move || {
                                link.send_message(ScaleGeneratorMsg::Stop);
                            });
                            self.play_timeout = Some(timeout);
                        }
                    } else {
                        // SET_INTERVAL 노트는 실제로 재생하지 않고 다음 노트로 진행
                        self.current_note_idx = next_idx;
                        
                        // 다음 노트로 바로 진행 (BPM 기반으로는 추가 딜레이 없음)
                        let link = ctx.link().clone();
                        link.send_message(ScaleGeneratorMsg::PlayNextNote);
                    }
                } else {
                    // 마지막 노트까지 재생 완료
                    self.playback_state = PlaybackState::Stopped;
                    self.current_note_idx = 0;
                    self.current_playing_note = None;
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
            ScaleGeneratorMsg::ClearIntervals => {
                self.intervals.clear();
                self.intervals.push("1".to_string());
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let notes = vec![
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"
        ];
        
        // 옥타브 범위 함수 - 음 이름에 따라 선택 가능한 옥타브 범위 반환
        let get_octave_range = |note_name: &str| -> Vec<i32> {
            match note_name {
                "A" | "A#" | "B" => (0..=7).collect(), // A, A#, B는 0~7 옥타브
                "C" => (1..=8).collect(),              // C는 1~8 옥타브
                _ => (1..=7).collect(),                // 나머지는 1~7 옥타브
            }
        };
        
        // 시작 음에 대한 옥타브 범위
        let start_octaves = get_octave_range(&self.start_note.name);
        
        // 종료 음에 대한 옥타브 범위
        let end_octaves = get_octave_range(&self.end_note.name);
        
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
                    <div class="generator-layout">
                        <div class="left-column">
                            <div class="note-settings">
                                <div class="note-settings-row">
                                    <div class="note-setting-group">
                                        <div class="note-setting-label">{"시작 근음:"}</div>
                                        <div class="note-setting-controls">
                                            <select
                                                value={self.start_note.name.clone()}
                                                onchange={ctx.link().callback(|e: Event| {
                                                    let select = e.target_dyn_into::<web_sys::HtmlSelectElement>().unwrap();
                                                    let name = select.value();
                                                    
                                                    // 음에 따른 기본 옥타브 설정
                                                    let default_octave = match name.as_str() {
                                                        "A" | "A#" | "B" => 0,   // A, A#, B는 기본 옥타브 0
                                                        "C" => 1,               // C는 기본 옥타브 1
                                                        _ => 1,                 // 나머지는 기본 옥타브 1
                                                    };
                                                    
                                                    ScaleGeneratorMsg::SetStartNote(name, default_octave)
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
                                                    start_octaves.iter().map(|&octave| {
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
                                    
                                    <div class="note-setting-group">
                                        <div class="note-setting-label">{"종료 근음:"}</div>
                                        <div class="note-setting-controls">
                                            <select
                                                value={self.end_note.name.clone()}
                                                onchange={
                                                    ctx.link().callback(|e: Event| {
                                                        let select = e.target_dyn_into::<web_sys::HtmlSelectElement>().unwrap();
                                                        let name = select.value();
                                                        
                                                        // 음에 따른 기본 옥타브 설정
                                                        let default_octave = match name.as_str() {
                                                            "A" | "A#" | "B" => 0,   // A, A#, B는 기본 옥타브 0
                                                            "C" => 1,               // C는 기본 옥타브 1
                                                            _ => 1,                 // 나머지는 기본 옥타브 1
                                                        };
                                                        
                                                        ScaleGeneratorMsg::SetEndNote(name, default_octave)
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
                                                    end_octaves.iter().map(|&octave| {
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
                            
                            <div class="direction-settings">
                                <div class="direction-label">{"재생 방향:"}</div>
                                <div class="radio-group">
                                    <div>
                                        <input 
                                            type="radio" 
                                            id="ascending"
                                            name="play-direction" 
                                            checked={self.play_direction == PlayDirection::Ascending}
                                            onchange={ctx.link().callback(|_| ScaleGeneratorMsg::SetPlayDirection(PlayDirection::Ascending))}
                                        />
                                        <label for="ascending">{"상행만"}</label>
                                    </div>
                                    
                                    <div>
                                        <input 
                                            type="radio" 
                                            id="both"
                                            name="play-direction" 
                                            checked={self.play_direction == PlayDirection::Both}
                                            onchange={ctx.link().callback(|_| ScaleGeneratorMsg::SetPlayDirection(PlayDirection::Both))}
                                        />
                                        <label for="both">{"상행/하행"}</label>
                                    </div>
                                    
                                    <div>
                                        <input 
                                            type="radio" 
                                            id="both-desc-first"
                                            name="play-direction" 
                                            checked={self.play_direction == PlayDirection::BothDescendingFirst}
                                            onchange={ctx.link().callback(|_| ScaleGeneratorMsg::SetPlayDirection(PlayDirection::BothDescendingFirst))}
                                        />
                                        <label for="both-desc-first">{"하행/상행"}</label>
                                    </div>
                                    
                                    <div>
                                        <input 
                                            type="radio" 
                                            id="descending"
                                            name="play-direction" 
                                            checked={self.play_direction == PlayDirection::Descending}
                                            onchange={ctx.link().callback(|_| ScaleGeneratorMsg::SetPlayDirection(PlayDirection::Descending))}
                                        />
                                        <label for="descending">{"하행만"}</label>
                                    </div>
                                </div>
                            </div>
                        </div>
                        
                        <div class="right-column">
                            <div class="intervals-container scale-intervals-container">
                                <div class="intervals-header">
                                    <div class="intervals-title">{"스케일 인터벌"}</div>
                                    <div class="interval-buttons">
                                        <button
                                            class="clear-intervals"
                                            title="인터벌 초기화 (근음만 남김)"
                                            onclick={ctx.link().callback(|_| {
                                                // 근음만 남기고 모든 인터벌 삭제
                                                ScaleGeneratorMsg::ClearIntervals
                                            })}
                                        >
                                            {"초기화"}
                                        </button>
                                        <button
                                            class="add-interval"
                                            title="인터벌 추가"
                                            onclick={ctx.link().callback(|_| ScaleGeneratorMsg::AddInterval)}
                                        >
                                            {"+"}
                                        </button>
                                    </div>
                                </div>
                                <div class="intervals-scroll">
                                    {
                                        self.intervals.iter().enumerate().map(|(idx, interval)| {
                                            html! {
                                                <div class="interval-item">
                                                    <div class="interval-index">{format!("#{}", idx + 1)}</div>
                                                    <select
                                                        class="interval-select"
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
                                                                    title="인터벌 제거"
                                                                    onclick={ctx.link().callback(move |_| ScaleGeneratorMsg::RemoveInterval(idx))}
                                                                >
                                                                    {"×"}
                                                                </button>
                                                            }
                                                        } else {
                                                            html! {
                                                                <div class="placeholder-button"></div>
                                                            }
                                                        }
                                                    }
                                                </div>
                                            }
                                        }).collect::<Html>()
                                    }
                                </div>
                            </div>
                        </div>
                    </div>
                    
                    <div class="bottom-controls">
                        <div class="bpm-controls">
                            <div class="bpm-buttons left">
                                <button 
                                    onclick={
                                        let current_bpm = self.bpm;
                                        ctx.link().callback(move |_| {
                                            let new_bpm = if current_bpm <= 35 { 30 } else { current_bpm - 5 };
                                            ScaleGeneratorMsg::SetBpm(new_bpm)
                                        })
                                    }
                                    disabled={self.bpm <= 30}
                                    title="BPM 5 감소"
                                >
                                    {"- 5"}
                                </button>
                                
                                <button 
                                    onclick={
                                        let current_bpm = self.bpm;
                                        ctx.link().callback(move |_| {
                                            let new_bpm = if current_bpm <= 30 { 30 } else { current_bpm - 1 };
                                            ScaleGeneratorMsg::SetBpm(new_bpm)
                                        })
                                    }
                                    disabled={self.bpm <= 30}
                                    title="BPM 1 감소"
                                >
                                    {"-"}
                                </button>
                            </div>
                            
                            <div class="bpm-value-display">
                                <span class="bpm-label">{"BPM:"}</span>
                                <span class="bpm-value">{self.bpm}</span>
                            </div>
                            
                            <div class="bpm-buttons right">
                                <button 
                                    onclick={
                                        let current_bpm = self.bpm;
                                        ctx.link().callback(move |_| {
                                            let new_bpm = if current_bpm >= 300 { 300 } else { current_bpm + 1 };
                                            ScaleGeneratorMsg::SetBpm(new_bpm)
                                        })
                                    }
                                    disabled={self.bpm >= 300}
                                    title="BPM 1 증가"
                                >
                                    {"+"}
                                </button>
                                
                                <button 
                                    onclick={
                                        let current_bpm = self.bpm;
                                        ctx.link().callback(move |_| {
                                            let new_bpm = if current_bpm >= 295 { 300 } else { current_bpm + 5 };
                                            ScaleGeneratorMsg::SetBpm(new_bpm)
                                        })
                                    }
                                    disabled={self.bpm >= 300}
                                    title="BPM 5 증가"
                                >
                                    {"+ 5"}
                                </button>
                            </div>
                        </div>
                        
                        <div class="current-note-display">
                            <div class="note-display-item">
                                {
                                    if let Some(root_note) = &self.current_root_note {
                                        html! {
                                            <>
                                                <span class="note-label">{"현재 근음:"}</span>
                                                <span class="note-value">{root_note.full_name()}</span>
                                            </>
                                        }
                                    } else {
                                        html! {
                                            <>
                                                <span class="note-label">{"현재 근음:"}</span>
                                                <span class="note-value note-waiting">{"대기 중"}</span>
                                            </>
                                        }
                                    }
                                }
                            </div>
                            <div class="note-display-item">
                                {
                                    if let Some(playing_note) = &self.current_playing_note {
                                        html! {
                                            <>
                                                <span class="note-label">{"현재 재생 음:"}</span>
                                                <span class="note-value">{playing_note.full_name()}</span>
                                            </>
                                        }
                                    } else {
                                        html! {
                                            <>
                                                <span class="note-label">{"현재 재생 음:"}</span>
                                                <span class="note-value note-waiting">{"대기 중"}</span>
                                            </>
                                        }
                                    }
                                }
                            </div>
                        </div>
                    
                        <div class="button-group">
                            <button
                                class={if self.playback_state == PlaybackState::Playing { "play-button playing" } else { "play-button" }}
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
                </div>
            </div>
        }
    }
}

impl ScaleGenerator {
    // 피아노 음원으로 노트 재생
    fn play_piano_note(&mut self, ctx: &Context<Self>, note: &Note) {
        // 문서 객체 모델에서 window 객체 가져오기
        let window = web_sys::window().expect("window 객체를 가져올 수 없습니다");
        let document = window.document().expect("document 객체를 가져올 수 없습니다");
        
        // 이전 오디오 요소 저장 (나중에 중지하기 위해)
        let prev_audio = self.audio_element.take();
        
        // 새 오디오 요소 생성
        let audio_element = match document.create_element("audio") {
            Ok(element) => element,
            Err(err) => {
                web_sys::console::error_1(&format!("오디오 요소 생성 실패: {:?}", err).into());
                return;
            }
        };
        
        let audio_element: HtmlAudioElement = audio_element
            .dyn_into::<HtmlAudioElement>()
            .expect("HtmlAudioElement로 변환할 수 없습니다");
        
        // 피아노 음원 파일 경로 설정
        let piano_file_path = note.piano_file_path();
        audio_element.set_src(&piano_file_path);
        
        // 볼륨 설정
        audio_element.set_volume(0.7);
        
        // 오디오 요소 저장
        self.audio_element = Some(audio_element.clone());
        
        // 오디오 요소를 미리 로드
        let _ = audio_element.load();
        
        // 시작 위치를 0초로 설정 후 재생
        audio_element.set_current_time(0.0);
        
        // 오디오 재생
        if let Err(err) = audio_element.play() {
            web_sys::console::error_1(&format!("오디오 재생 실패: {:?}", err).into());
        } else {
            web_sys::console::log_1(&format!("피아노 노트 재생: {} (파일: {})",
                note.full_name(), piano_file_path).into());
                
            // 이전 오디오가 있다면, 새 오디오가 재생된 후 0.1초 후에 중지
            if let Some(prev) = prev_audio {
                // 0.1초 후에 이전 오디오 중지
                let window_clone = window.clone();
                let closure = Closure::once_into_js(move || {
                    let _ = prev.pause();
                    let _ = prev.set_src("");  // 리소스 해제
                });
                
                let _ = window_clone.set_timeout_with_callback_and_timeout_and_arguments_0(
                    closure.as_ref().unchecked_ref(),
                    100  // 0.1초 (100ms)
                );
            }
        }
    }

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
            
            // 선택된 모든 인터벌에 대해 음 추가
            for interval in &self.intervals {
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