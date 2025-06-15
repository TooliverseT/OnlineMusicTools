use wasm_bindgen::prelude::*;
use web_sys::{AudioContext, AudioNode, AudioParam, GainNode, HtmlAudioElement, KeyboardEvent, Document};
use yew::prelude::*;
use std::collections::HashMap;
use std::rc::Rc;
use gloo_timers::callback::Timeout;
use wasm_bindgen::closure::Closure;
use web_sys::console;
use js_sys;
use log::info;

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

// 키보드 매핑 추가
#[derive(Clone, PartialEq)]
struct KeyMapping {
    keyboard_key: String,  // 키보드 키 (예: "a", "s", "d"...)
    piano_note: String,    // 피아노 노트 (예: "C", "C#", "D"...)
    is_left_hand: bool,    // 왼손/오른손 구분
    octave_offset: i32,    // 기본 옥타브에서의 오프셋
}

// 노트 이름 인덱스 (C = 0, C# = 1, ... B = 11)
const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

// 피아노 컴포넌트 메시지에 키보드 이벤트 추가
pub enum PianoMsg {
    KeyPressed(usize),              // 키가 눌렸을 때
    KeyReleased(usize),             // 키가 떼어졌을 때
    ToggleSustain,                  // 서스테인 토글
    StopSound(String),              // 특정 소리 정지
    SetStartOctave(i32),            // 시작 옥타브 설정
    ScrollPiano(i32),               // 피아노 스크롤
    KeyboardKeyDown(String),        // 키보드 키가 눌렸을 때
    KeyboardKeyUp(String),          // 키보드 키가 떼어졌을 때
    ChangeLeftHandOctave(i32),      // 왼손 옥타브 변경
    ChangeRightHandOctave(i32),     // 오른손 옥타브 변경
    MovePianoUIRange(i32),          // 피아노 UI 표시 범위 이동
    ChangeStartNote(i32),           // 왼손과 오른손 시작 음 동시 변경
    ChangeLeftHandStartNote(i32),      // 왼손 시작 음 변경
    ChangeRightHandStartNote(i32),     // 오른손 시작 음 변경
    ResetAllKeys,                   // 모든 키를 리셋
    ForceKeyUpdate,                 // 키 상태 강제 업데이트
    ToggleKeyboardInput,            // 키보드 입력 활성화/비활성화 토글
    PlaySet(usize),                 // 피아노 세트 재생
    ReleaseSet(usize),              // 피아노 세트 재생 중지
    ToggleSetEditMode,              // 세트 수정 모드 토글
    SelectSetToEdit(usize),         // 수정할 세트 선택
    ToggleKeyInSet(usize),          // 세트에서 키 토글 (추가/제거)
    ToggleKeyInSetWithSound(usize),           // 소리와 함께 세트에서 키 토글
    ClearAllSets,                   // 모든 세트 초기화
    StopSetSounds(usize),           // 세트의 모든 소리 정지
    RemoveSetSound(usize, usize),   // 특정 세트의 특정 키 소리 제거
    StopSetSoundsIfReleased(usize),  // 세트의 모든 키가 눌려있지 않고 서스테인이 꺼져 있을 때만 소리 정지
    StopSetKeySound(usize, usize),   // 특정 세트의 특정 키 소리 정지
    AddActiveSound(String, HtmlAudioElement), // 활성 소리 추가
    RemoveActiveSound(String),        // 활성 소리 제거
    FadeOutSound(String, f64),      // 특정 소리를 서서히 페이드아웃 (소리 이름, 현재 볼륨)
}

// 피아노 컴포넌트
pub struct PianoKeyboard {
    keys: Vec<PianoKey>,            // 모든 피아노 키
    active_sounds: HashMap<String, HtmlAudioElement>, // 현재 재생 중인 소리
    sustain: bool,                  // 서스테인 상태
    start_octave: i32,              // 표시할 시작 옥타브
    audio_ctx: Option<AudioContext>, // 오디오 컨텍스트
    key_mappings: Vec<KeyMapping>,  // 키보드 매핑 정보
    left_hand_octave: i32,          // 왼손 옥타브 (기본 C2-C3)
    right_hand_octave: i32,         // 오른손 옥타브 (기본 C4-C5)
    left_hand_start_note_idx: usize, // 왼손 시작 음 인덱스 (0 = C, 1 = C#, ...)
    right_hand_start_note_idx: usize, // 오른손 시작 음 인덱스
    pressed_keyboard_keys: HashMap<String, bool>, // 현재 눌려있는 키보드 키
    _keyboard_listeners: Option<(
        Closure<dyn FnMut(KeyboardEvent)>,  // keydown
        Closure<dyn FnMut(KeyboardEvent)>,  // keyup
        Closure<dyn FnMut(web_sys::Event)>, // blur
        Closure<dyn FnMut(web_sys::FocusEvent)>, // focusout
        Closure<dyn FnMut(web_sys::MouseEvent)>, // mouseleave
        Closure<dyn FnMut(web_sys::Event)>, // visibilitychange
    )>, // 키보드 이벤트 리스너들
    keyboard_input_enabled: bool,   // 키보드 입력 활성화 여부
    piano_sets: Vec<Vec<usize>>,    // 피아노 세트 (키 인덱스의 집합)
    set_edit_mode: bool,            // 세트 수정 모드 활성화 여부
    current_edit_set: Option<usize>, // 현재 수정 중인 세트 인덱스
    active_set: Option<usize>,      // 현재 활성화된 세트 인덱스
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

        // 키보드 매핑 초기화
        let mut key_mappings = Vec::new();
        let mut pressed_keyboard_keys = HashMap::new();
        
        // 왼손 키 매핑 (C2-C3 기본 옥타브)
        let left_hand_keys = ["z", "x", "c", "v", "a", "s", "d", "f", "w", "e", "r", "t", "y"];
        
        // 오른손 키 매핑 (C4-C5 기본 옥타브)
        let right_hand_keys = ["m", ",", ".", "/", "j", "k", "l", ";", "u", "i", "o", "p", "["];
        
        // 초기 매핑 생성
        let left_hand_start_note_idx = 0; // C로 시작
        let right_hand_start_note_idx = 0; // C로 시작
        
        Self::create_key_mappings(
            &mut key_mappings, 
            &mut pressed_keyboard_keys, 
            &left_hand_keys, 
            &right_hand_keys, 
            left_hand_start_note_idx, 
            right_hand_start_note_idx
        );
        
        // 옥타브 변경 키 매핑
        pressed_keyboard_keys.insert("b".to_string(), false); // 왼손 옥타브 내림
        pressed_keyboard_keys.insert("g".to_string(), false); // 왼손 옥타브 올림
        pressed_keyboard_keys.insert("n".to_string(), false); // 오른손 옥타브 내림
        pressed_keyboard_keys.insert("h".to_string(), false); // 오른손 옥타브 올림
        pressed_keyboard_keys.insert("q".to_string(), false); // UI 범위 한 옥타브 아래로
        pressed_keyboard_keys.insert("]".to_string(), false); // UI 범위 한 옥타브 위로
        pressed_keyboard_keys.insert(" ".to_string(), false); // 스페이스바 (서스테인)
        pressed_keyboard_keys.insert("'".to_string(), false); // 작은따옴표 (키보드 입력 활성화/비활성화)
        pressed_keyboard_keys.insert("-".to_string(), false); // - (왼손 시작 음 높이기)
        pressed_keyboard_keys.insert("=".to_string(), false); // = (오른손 시작 음 높이기)
        pressed_keyboard_keys.insert("_".to_string(), false); // _ (왼손 시작 음 낮추기)
        pressed_keyboard_keys.insert("+".to_string(), false); // + (오른손 시작 음 낮추기)
        pressed_keyboard_keys.insert("0".to_string(), false); // 0 (매핑 초기화)
        pressed_keyboard_keys.insert("~".to_string(), false); // ~ (전체 세트 초기화)
        
        // 세트 키 매핑 (1-0)
        pressed_keyboard_keys.insert("1".to_string(), false); // 1번 세트
        pressed_keyboard_keys.insert("2".to_string(), false); // 2번 세트
        pressed_keyboard_keys.insert("3".to_string(), false); // 3번 세트
        pressed_keyboard_keys.insert("4".to_string(), false); // 4번 세트
        pressed_keyboard_keys.insert("5".to_string(), false); // 5번 세트
        pressed_keyboard_keys.insert("6".to_string(), false); // 6번 세트
        pressed_keyboard_keys.insert("7".to_string(), false); // 7번 세트
        pressed_keyboard_keys.insert("8".to_string(), false); // 8번 세트
        pressed_keyboard_keys.insert("9".to_string(), false); // 9번 세트
        pressed_keyboard_keys.insert("0".to_string(), false); // 10번 세트
        pressed_keyboard_keys.insert("`".to_string(), false); // 수정 모드 토글

        // 피아노 세트 초기화 (10개의 빈 세트)
        let mut piano_sets = Vec::new();
        for _ in 0..10 {
            piano_sets.push(Vec::new());
        }

        Self {
            keys,
            active_sounds: HashMap::new(),
            sustain: false,
            start_octave: 2, // 기본 시작 옥타브는 2
            audio_ctx: None,
            key_mappings,
            left_hand_octave: 2,
            right_hand_octave: 4,
            left_hand_start_note_idx,
            right_hand_start_note_idx,
            pressed_keyboard_keys,
            _keyboard_listeners: None,
            keyboard_input_enabled: false,
            piano_sets,
            set_edit_mode: false,
            current_edit_set: None,
            active_set: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            PianoMsg::KeyPressed(index) => {
                if index < self.keys.len() {
                    self.keys[index].is_pressed = true;
                    
                    // 동일한 키에 대한 이전 소리 제거 (연타 방지를 위함)
                    let key_base_name = self.keys[index].full_name();
                    // 해당 키에 관련된 모든 소리 찾기
                    let existing_sounds: Vec<String> = self.active_sounds.keys()
                        .filter(|k| k.starts_with(&key_base_name))
                        .cloned()
                        .collect();
                    
                    // 기존 소리를 중지하지 않고 페이드아웃하도록 변경
                    for key_name in existing_sounds {
                        if let Some(audio) = self.active_sounds.get(&key_name) {
                            // 현재 볼륨 값을 가져와 페이드아웃 시작
                            let current_volume = audio.volume();
                            let key_name_clone = key_name.clone();
                            let link = ctx.link().clone();
                            
                            // 페이드아웃 메시지 전송
                            link.send_message(PianoMsg::FadeOutSound(key_name_clone, current_volume));
                        }
                    }
                    
                    // 약간의 지연 후 새 오디오 요소 생성 및 재생
                    let audio_path = self.keys[index].audio_path();
                    let key_full_name = self.keys[index].full_name();
                    let link = ctx.link().clone();
                    
                    // 10ms 지연 후 새 오디오 생성 및 재생
                    let timeout = Timeout::new(10, move || {
                        // 새 오디오 요소 생성
                        if let Ok(audio) = HtmlAudioElement::new_with_src(&audio_path) {
                            // 볼륨 설정
                            audio.set_volume(0.7);
                            
                            // 시작 위치 리셋
                            audio.set_current_time(0.0);
                            
                            // 오디오 요소 미리 로드
                            let _ = audio.load();
                            
                            // 고유 ID 생성 (타임스탬프 추가)
                            let key_name = format!("{}_{}", key_full_name, js_sys::Date::now());
                            
                            // 먼저 재생하려면 타임스탬프 지연이 중요함
                            let play_link = link.clone();
                            let key_name_clone = key_name.clone();
                            let audio_clone = audio.clone();
                            
                            // active_sounds에 추가
                            let msg = PianoMsg::AddActiveSound(key_name_clone, audio_clone);
                            play_link.send_message(msg);
                            
                            // 약간의 지연 후 재생 시작
                            let play_timeout = Timeout::new(5, move || {
                                match audio.play() {
                                    Ok(_) => {
                                        console::log_1(&format!("피아노 노트 재생: {}", key_name).into());
                                    },
                                    Err(err) => {
                                        console::error_1(&format!("오디오 재생 실패: {:?}", err).into());
                                        // 재생 실패 시 맵에서 제거
                                        play_link.send_message(PianoMsg::RemoveActiveSound(key_name));
                                    }
                                }
                            });
                            play_timeout.forget();
                        }
                    });
                    timeout.forget();
                    
                    true
                } else {
                    false
                }
            },
            PianoMsg::KeyReleased(index) => {
                if index < self.keys.len() {
                    // 이미 뗀 상태면 무시
                    if !self.keys[index].is_pressed {
                        return false;
                    }
                    
                    self.keys[index].is_pressed = false;
                    
                    // 서스테인이 꺼져 있으면 0.5초 후에 해당 키의 모든 소리 정지
                    if !self.sustain {
                        let key_base_name = self.keys[index].full_name();
                        
                        // 해당 키에 관련된 모든 소리 찾기 (타임스탬프 무관)
                        let sounds_to_stop: Vec<String> = self.active_sounds.keys()
                            .filter(|k| k.starts_with(&key_base_name))
                            .cloned()
                            .collect();
                        
                        for key_name in sounds_to_stop {
                            let key_name_clone = key_name.clone();
                            let link = ctx.link().clone();
                            
                            // 0.5초 후에 소리 정지
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
                    // 일반 키에 대한 처리
                    let keys_to_stop: Vec<String> = self.active_sounds.keys()
                        .filter(|k| {
                            // 키 이름에서 타임스탬프 부분 제거 (첫 번째 '_' 앞부분만 사용)
                            let base_name = if let Some(pos) = k.find('_') {
                                &k[0..pos]
                            } else {
                                k
                            };
                            
                            // 해당 베이스 이름을 가진 키가 눌려있는지 확인
                            !self.keys.iter().any(|key| key.is_pressed && k.starts_with(&key.full_name()))
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
                // 소리를 먼저 제거하고 나중에 일시 중지 - 재생 중단 오류 방지
                if let Some(audio) = self.active_sounds.remove(&key_name) {
                    // 맵에서 먼저 제거한 후 pause 호출
                    let _ = audio.set_current_time(0.0);
                    let _ = audio.pause();
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
            },
            PianoMsg::KeyboardKeyDown(key) => {
                // 키보드 입력이 비활성화된 경우 무시
                if !self.keyboard_input_enabled {
                    // 작은따옴표(') 키는 키보드 입력 활성화/비활성화 토글로 항상 처리
                    if key == "'" {
                        return yew::Component::update(self, ctx, PianoMsg::ToggleKeyboardInput);
                    }
                    return false;
                }
                
                // 옥타브 변경 키 처리
                match key.as_str() {
                    "-" => return yew::Component::update(self, ctx, PianoMsg::ChangeLeftHandOctave(-1)), // 왼손 옥타브 내림 (이전: b)
                    "_" => return yew::Component::update(self, ctx, PianoMsg::ChangeLeftHandOctave(1)), // 왼손 옥타브 올림 (이전: g)
                    "=" => return yew::Component::update(self, ctx, PianoMsg::ChangeRightHandOctave(-1)), // 오른손 옥타브 내림 (이전: n)
                    "+" => return yew::Component::update(self, ctx, PianoMsg::ChangeRightHandOctave(1)), // 오른손 옥타브 올림 (이전: h)
                    "q" => return yew::Component::update(self, ctx, PianoMsg::MovePianoUIRange(-1)), // UI 범위를 한 옥타브 아래로
                    "]" => return yew::Component::update(self, ctx, PianoMsg::MovePianoUIRange(1)),  // UI 범위를 한 옥타브 위로
                    " " => {
                        // 스페이스바를 누르면 서스테인 활성화
                        if !self.sustain {
                            return yew::Component::update(self, ctx, PianoMsg::ToggleSustain);
                        }
                        return false;
                    },
                    "'" => {
                        // 작은따옴표(') 키를 누르면 키보드 입력 활성화/비활성화 토글
                        return yew::Component::update(self, ctx, PianoMsg::ToggleKeyboardInput);
                    },
                    "b" => {
                        // 왼손 시작 음 낮추기 (이전: -)
                        return yew::Component::update(self, ctx, PianoMsg::ChangeLeftHandStartNote(-1));
                    },
                    "g" => {
                        // 왼손 시작 음 높이기 (이전: _)
                        return yew::Component::update(self, ctx, PianoMsg::ChangeLeftHandStartNote(1));
                    },
                    "n" => {
                        // 오른손 시작 음 낮추기 (이전: =)
                        return yew::Component::update(self, ctx, PianoMsg::ChangeRightHandStartNote(-1));
                    },
                    "h" => {
                        // 오른손 시작 음 높이기 (이전: +)
                        return yew::Component::update(self, ctx, PianoMsg::ChangeRightHandStartNote(1));
                    },
                    "Escape" => {
                        // Escape 키를 누르면 모든 키 리셋
                        return yew::Component::update(self, ctx, PianoMsg::ResetAllKeys);
                    },
                    "`" => {
                        // ` 키를 누르면 수정 모드 토글
                        return yew::Component::update(self, ctx, PianoMsg::ToggleSetEditMode);
                    },
                    "~" => {
                        // ~ 키를 누르면 모든 세트 초기화
                        return yew::Component::update(self, ctx, PianoMsg::ClearAllSets);
                    },
                    _ => {}
                }
                
                // 이미 눌려있는 키는 무시
                if let Some(is_pressed) = self.pressed_keyboard_keys.get_mut(&key) {
                    if *is_pressed {
                        return false;
                    }
                    *is_pressed = true;
                    
                    // 매핑된 피아노 키 찾기
                    if let Some(piano_key_idx) = self.find_piano_key_by_keyboard(&key) {
                        // 수정 모드면 해당 키를 토글하고, 아니면 소리 재생
                        if self.set_edit_mode && self.current_edit_set.is_some() {
                            return yew::Component::update(self, ctx, PianoMsg::ToggleKeyInSetWithSound(piano_key_idx));
                        } else {
                            return yew::Component::update(self, ctx, PianoMsg::KeyPressed(piano_key_idx));
                        }
                    }
                }
                false
            },
            PianoMsg::KeyboardKeyUp(key) => {
                // 키보드 입력이 비활성화된 경우 무시
                if !self.keyboard_input_enabled {
                    // 작은따옴표(') 키는 키보드 입력 활성화/비활성화 토글로 항상 처리 (키업은 이미 처리됨)
                    if key == "'" {
                        if let Some(is_pressed) = self.pressed_keyboard_keys.get_mut(&key) {
                            *is_pressed = false;
                        }
                        return false;
                    }
                    return false;
                }
                
                // 옥타브 변경 키는 별도 처리 필요 없음
                match key.as_str() {
                    " " => {
                        // 스페이스바를 떼면 서스테인 비활성화
                        if self.sustain {
                            return yew::Component::update(self, ctx, PianoMsg::ToggleSustain);
                        }
                        return false;
                    },
                    "b" | "g" | "n" | "h" | "q" | "]" | "-" | "=" | "_" | "+" | "`" | "~" | "'" => {
                        if let Some(is_pressed) = self.pressed_keyboard_keys.get_mut(&key) {
                            *is_pressed = false;
                        }
                        
                        // 강제로 키 상태 업데이트 요청
                        let link = ctx.link().clone();
                        let timeout = Timeout::new(10, move || {
                            link.send_message(PianoMsg::ForceKeyUpdate);
                        });
                        timeout.forget();
                        
                        return false;
                    },
                    _ => {}
                }
                
                if let Some(is_pressed) = self.pressed_keyboard_keys.get_mut(&key) {
                    *is_pressed = false;
                    
                    // 매핑된 피아노 키 찾기
                    if let Some(piano_key_idx) = self.find_piano_key_by_keyboard(&key) {
                        let result = yew::Component::update(self, ctx, PianoMsg::KeyReleased(piano_key_idx));
                        
                        // 강제로 키 상태 업데이트 요청 (지연 설정)
                        let link = ctx.link().clone();
                        let timeout = Timeout::new(10, move || {
                            link.send_message(PianoMsg::ForceKeyUpdate);
                        });
                        timeout.forget();
                        
                        return result;
                    }
                }
                false
            },
            PianoMsg::MovePianoUIRange(delta) => {
                return yew::Component::update(self, ctx, PianoMsg::ScrollPiano(delta));
            },
            PianoMsg::ChangeLeftHandOctave(delta) => {
                let new_octave = self.left_hand_octave + delta;
                if new_octave >= 0 && new_octave <= 7 { // 최대 C7까지 가능하도록 변경
                    let old_octave = self.left_hand_octave;
                    self.left_hand_octave = new_octave;
                    
                    // 영역이 바뀌면 이전 영역에 눌려있던 키들 해제
                    self.release_keys_in_octave(ctx, old_octave, true);
                    
                    true
                } else {
                    false
                }
            },
            PianoMsg::ChangeRightHandOctave(delta) => {
                let new_octave = self.right_hand_octave + delta;
                if new_octave >= 0 && new_octave <= 7 { // 최대 C7까지 가능하도록 변경
                    let old_octave = self.right_hand_octave;
                    self.right_hand_octave = new_octave;
                    
                    // 영역이 바뀌면 이전 영역에 눌려있던 키들 해제
                    self.release_keys_in_octave(ctx, old_octave, false);
                    
                    true
                } else {
                    false
                }
            },
            PianoMsg::ChangeStartNote(delta) => {
                if delta > 0 {
                    // 양수일 때는 오른손만 변경
                    let mut new_idx = self.right_hand_start_note_idx as i32 + delta;
                    if new_idx < 0 {
                        new_idx += 12;
                    } else if new_idx >= 12 {
                        new_idx -= 12;
                    }
                    self.right_hand_start_note_idx = new_idx as usize;
                } else {
                    // 음수일 때는 왼손만 변경
                    let mut new_idx = self.left_hand_start_note_idx as i32 + delta;
                    if new_idx < 0 {
                        new_idx += 12;
                    } else if new_idx >= 12 {
                        new_idx -= 12;
                    }
                    self.left_hand_start_note_idx = new_idx as usize;
                }
                
                // 키 매핑 재생성
                self.recreate_key_mappings();
                true
            },
            PianoMsg::ChangeLeftHandStartNote(delta) => {
                let old_idx = self.left_hand_start_note_idx;
                let mut new_idx = self.left_hand_start_note_idx as i32 + delta;
                let mut octave_change = 0;
                
                // 범위를 벗어났을 때 옥타브 변경 처리
                if new_idx < 0 {
                    new_idx += 12;
                    octave_change = -1; // 옥타브 다운
                } else if new_idx >= 12 {
                    new_idx -= 12;
                    octave_change = 1; // 옥타브 업
                }
                
                self.left_hand_start_note_idx = new_idx as usize;
                
                // 옥타브 변경이 필요한 경우 처리
                if octave_change != 0 {
                    let new_octave = self.left_hand_octave + octave_change;
                    if new_octave >= 0 && new_octave <= 7 {
                        self.left_hand_octave = new_octave;
                    }
                }
                
                // 영역이 바뀌면 이전 영역에 눌려있던 키들 해제
                self.release_keys_for_changed_note_idx(ctx, old_idx, self.left_hand_start_note_idx, self.left_hand_octave, true);
                
                // 키 매핑 재생성
                self.recreate_key_mappings();
                true
            },
            PianoMsg::ChangeRightHandStartNote(delta) => {
                let old_idx = self.right_hand_start_note_idx;
                let mut new_idx = self.right_hand_start_note_idx as i32 + delta;
                let mut octave_change = 0;
                
                // 범위를 벗어났을 때 옥타브 변경 처리
                if new_idx < 0 {
                    new_idx += 12;
                    octave_change = -1; // 옥타브 다운
                } else if new_idx >= 12 {
                    new_idx -= 12;
                    octave_change = 1; // 옥타브 업
                }
                
                self.right_hand_start_note_idx = new_idx as usize;
                
                // 옥타브 변경이 필요한 경우 처리
                if octave_change != 0 {
                    let new_octave = self.right_hand_octave + octave_change;
                    if new_octave >= 0 && new_octave <= 7 {
                        self.right_hand_octave = new_octave;
                    }
                }
                
                // 영역이 바뀌면 이전 영역에 눌려있던 키들 해제
                self.release_keys_for_changed_note_idx(ctx, old_idx, self.right_hand_start_note_idx, self.right_hand_octave, false);
                
                // 키 매핑 재생성
                self.recreate_key_mappings();
                true
            },
            PianoMsg::ResetAllKeys => {
                // 모든 키보드 키 상태 초기화
                for (_, is_pressed) in self.pressed_keyboard_keys.iter_mut() {
                    *is_pressed = false;
                }
                
                // 모든 피아노 키 상태 초기화
                for key in self.keys.iter_mut() {
                    key.is_pressed = false;
                }
                
                // 활성 세트 초기화
                self.active_set = None;
                
                // 모든 소리 중지
                let sounds_to_stop: Vec<String> = self.active_sounds.keys().cloned().collect();
                for key_name in sounds_to_stop {
                    if let Some(audio) = self.active_sounds.get(&key_name) {
                        let _ = audio.pause();
                        let _ = audio.set_current_time(0.0);
                        self.active_sounds.remove(&key_name);
                    }
                }
                
                true
            },
            PianoMsg::ForceKeyUpdate => {
                let mut updated = false;
                
                // 키보드 상태와 피아노 키 상태를 동기화
                for (i, key) in self.keys.iter_mut().enumerate() {
                    // 매핑된 키보드 키 찾기
                    let has_pressed_key = self.key_mappings.iter()
                        .filter(|mapping| {
                            let octave = if mapping.is_left_hand {
                                self.left_hand_octave + mapping.octave_offset
                            } else {
                                self.right_hand_octave + mapping.octave_offset
                            };
                            
                            key.name == mapping.piano_note && key.octave == octave
                        })
                        .any(|mapping| {
                            self.pressed_keyboard_keys.get(&mapping.keyboard_key)
                                .map(|&is_pressed| is_pressed)
                                .unwrap_or(false)
                        });
                    
                    // 세트에 속한 키인 경우도 검사 - 활성화된 세트가 있는 경우에만 세트 키를 눌려있게 표시
                    let is_in_active_set = if let Some(set_idx) = self.active_set {
                        self.piano_sets[set_idx].contains(&i)
                    } else {
                        false
                    };
                    
                    // 키 상태 불일치 수정 (세트에 속한 키는 항상 눌려있는 상태로 유지)
                    let should_be_pressed = has_pressed_key || is_in_active_set;
                    
                    if key.is_pressed != should_be_pressed {
                        key.is_pressed = should_be_pressed;
                        updated = true;
                        
                        // 눌려있지 않아야 하는데 눌려있으면 소리 중지
                        if !should_be_pressed {
                            let key_base_name = key.full_name();
                            
                            // 해당 키에 관련된 모든 소리 찾기 (타임스탬프 무관)
                            let sounds_to_stop: Vec<String> = self.active_sounds.keys()
                                .filter(|k| k.starts_with(&key_base_name))
                                .cloned()
                                .collect();
                            
                            for key_name in sounds_to_stop {
                                let key_name_clone = key_name.clone();
                                let link = ctx.link().clone();
                                
                                // 즉시 소리 정지
                                link.send_message(PianoMsg::StopSound(key_name_clone));
                            }
                        }
                    }
                }
                
                updated
            },
            PianoMsg::ToggleKeyboardInput => {
                self.keyboard_input_enabled = !self.keyboard_input_enabled;
                
                // 상태 변경 로그 출력
                console::log_1(&format!("키보드 입력 {}", if self.keyboard_input_enabled { "활성화" } else { "비활성화" }).into());
                
                true
            },
            PianoMsg::PlaySet(set_idx) => {
                if set_idx < self.piano_sets.len() {
                    if self.set_edit_mode {
                        // 수정 모드에서는 세트 선택
                        return yew::Component::update(self, ctx, PianoMsg::SelectSetToEdit(set_idx));
                    }

                    // 현재 세트를 활성화된 세트로 설정
                    self.active_set = Some(set_idx);
                    
                    // 세트에 포함된 모든 키를 동시에 누름
                    for &key_idx in &self.piano_sets[set_idx] {
                        if key_idx < self.keys.len() {
                            // 키 상태 업데이트
                            self.keys[key_idx].is_pressed = true;
                            
                            // 동일한 키에 대한 이전 소리 제거 (연타 방지를 위함)
                            let key_base_name = self.keys[key_idx].full_name();
                            
                            // 이전 소리를 페이드아웃
                            let existing_sounds: Vec<String> = self.active_sounds.keys()
                                .filter(|k| k.starts_with(&key_base_name))
                                .cloned()
                                .collect();
                            
                            // 소리가 최소 500ms 재생되도록 타임스탬프 확인
                            for key_name in existing_sounds {
                                if let Some(audio) = self.active_sounds.get(&key_name) {
                                    // 키 이름에서 타임스탬프 추출
                                    if let Some(pos) = key_name.rfind('_') {
                                        if let Ok(timestamp) = key_name[pos+1..].parse::<f64>() {
                                            let current_time = js_sys::Date::now();
                                            let elapsed = current_time - timestamp;
                                            
                                            // 500ms 미만인 경우 페이드아웃 사용, 그렇지 않은 경우 기존 로직 사용
                                            if elapsed < 500.0 {
                                                // 현재 볼륨 값을 가져와 페이드아웃 시작
                                                let current_volume = audio.volume();
                                                let key_name_clone = key_name.clone();
                                                let link = ctx.link().clone();
                                                
                                                // 페이드아웃 메시지 전송 (500ms - elapsed 시간 후에 소리 정지)
                                                let remaining = (500.0 - elapsed).max(100.0) as u32;
                                                let timeout = Timeout::new(remaining, move || {
                                                    link.send_message(PianoMsg::FadeOutSound(key_name_clone, current_volume));
                                                });
                                                timeout.forget();
                                                continue;
                                            }
                                        }
                                    }
                                    
                                    // 기본 동작: 현재 볼륨 값을 가져와 페이드아웃 시작
                                    let current_volume = audio.volume();
                                    let key_name_clone = key_name.clone();
                                    let link = ctx.link().clone();
                                    
                                    // 페이드아웃 메시지 전송
                                    link.send_message(PianoMsg::FadeOutSound(key_name_clone, current_volume));
                                }
                            }
                            
                            // 약간의 지연 후 새 오디오 요소 생성 및 재생
                            let audio_path = self.keys[key_idx].audio_path();
                            let key_full_name = self.keys[key_idx].full_name();
                            let set_idx_copy = set_idx;
                            let link = ctx.link().clone();
                            
                            // 10ms 지연 후 새 오디오 생성 및 재생
                            let timeout = Timeout::new(10, move || {
                                // 새 오디오 요소 생성
                                if let Ok(audio) = HtmlAudioElement::new_with_src(&audio_path) {
                                    // 볼륨 설정
                                    audio.set_volume(0.7);
                                    
                                    // 시작 위치 리셋
                                    audio.set_current_time(0.0);
                                    
                                    // 오디오 요소 미리 로드
                                    let _ = audio.load();
                                    
                                    // 고유 ID 생성 (타임스탬프 추가)
                                    let key_name = format!("{}_{}", key_full_name, js_sys::Date::now());
                                    
                                    // 먼저 재생하려면 타임스탬프 지연이 중요함
                                    let play_link = link.clone();
                                    let key_name_clone = key_name.clone();
                                    let audio_clone = audio.clone();
                                    
                                    // active_sounds에 추가
                                    let msg = PianoMsg::AddActiveSound(key_name_clone, audio_clone);
                                    play_link.send_message(msg);
                                    
                                    // 약간의 지연 후 재생 시작
                                    let play_timeout = Timeout::new(5, move || {
                                        match audio.play() {
                                            Ok(_) => {
                                                console::log_1(&format!("피아노 노트 재생(세트{}): {}", set_idx_copy, key_name).into());
                                            },
                                            Err(err) => {
                                                console::error_1(&format!("오디오 재생 실패: {:?}", err).into());
                                                // 재생 실패 시 맵에서 제거
                                                play_link.send_message(PianoMsg::RemoveActiveSound(key_name));
                                            }
                                        }
                                    });
                                    play_timeout.forget();
                                }
                            });
                            timeout.forget();
                        }
                    }
                    
                    true
                } else {
                    false
                }
            },
            PianoMsg::ReleaseSet(set_idx) => {
                if set_idx < self.piano_sets.len() {
                    // 현재 활성화된 세트가 이 세트인 경우 활성화 상태 해제
                    if self.active_set == Some(set_idx) {
                        self.active_set = None;
                    }
                    
                    // 세트에 포함된 모든 키를 동시에 뗌
                    for &key_idx in &self.piano_sets[set_idx] {
                        // 이미 뗀 상태면 무시
                        if !self.keys[key_idx].is_pressed {
                            continue;
                        }
                        
                        self.keys[key_idx].is_pressed = false;
                        
                        // 서스테인이 꺼져 있으면 0.5초 후에 해당 키의 모든 소리 정지
                        if !self.sustain {
                            let key_base_name = self.keys[key_idx].full_name();
                            
                            // 해당 키에 관련된 모든 소리 찾기 (타임스탬프 무관)
                            let sounds_to_stop: Vec<String> = self.active_sounds.keys()
                                .filter(|k| k.starts_with(&key_base_name))
                                .cloned()
                                .collect();
                            
                            for key_name in sounds_to_stop {
                                let key_name_clone = key_name.clone();
                                let link = ctx.link().clone();
                                
                                // 0.5초 후에 소리 정지
                                let timeout = Timeout::new(500, move || {
                                    link.send_message(PianoMsg::StopSound(key_name_clone));
                                });
                                
                                // 타임아웃이 가비지 컬렉션되지 않도록 함
                                timeout.forget();
                            }
                        }
                    }
                    
                    true
                } else {
                    false
                }
            },
            PianoMsg::ToggleSetEditMode => {
                self.set_edit_mode = !self.set_edit_mode;
                
                // 수정 모드를 끄면 현재 편집 중인 세트도 리셋
                if !self.set_edit_mode {
                    self.current_edit_set = None;
                }
                
                true
            },
            PianoMsg::SelectSetToEdit(set_idx) => {
                if set_idx < self.piano_sets.len() {
                    // 같은 세트를 다시 선택하면 선택 취소
                    if self.current_edit_set == Some(set_idx) {
                        self.current_edit_set = None;
                    } else {
                        self.current_edit_set = Some(set_idx);
                    }
                    true
                } else {
                    false
                }
            },
            PianoMsg::ToggleKeyInSet(key_idx) => {
                if let Some(set_idx) = self.current_edit_set {
                    if key_idx < self.keys.len() {
                        // 이미 세트에 있는 키면 제거, 없으면 추가
                        if let Some(pos) = self.piano_sets[set_idx].iter().position(|&k| k == key_idx) {
                            self.piano_sets[set_idx].remove(pos);
                        } else {
                            self.piano_sets[set_idx].push(key_idx);
                        }
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            },
            PianoMsg::ToggleKeyInSetWithSound(key_idx) => {
                // 먼저 소리 재생
                let _ = yew::Component::update(self, ctx, PianoMsg::KeyPressed(key_idx));
                
                // 키 토글 처리
                let result = yew::Component::update(self, ctx, PianoMsg::ToggleKeyInSet(key_idx));
                
                // 약간의 시간 후에 키를 뗌
                let link = ctx.link().clone();
                let timeout = Timeout::new(200, move || {
                    link.send_message(PianoMsg::KeyReleased(key_idx));
                });
                timeout.forget();
                
                result
            },
            PianoMsg::ClearAllSets => {
                // 모든 세트 초기화
                for set in self.piano_sets.iter_mut() {
                    set.clear();
                }
                true
            },
            PianoMsg::StopSetSounds(set_idx) => {
                self.stop_set_sounds(set_idx);
                false
            },
            PianoMsg::RemoveSetSound(set_idx, key_idx) => {
                if set_idx < self.piano_sets.len() && key_idx < self.keys.len() {
                    let key_base_name = self.keys[key_idx].full_name();
                    
                    // 해당 키에 관련된 모든 소리 찾기 (타임스탬프 무관)
                    let sounds_to_stop: Vec<String> = self.active_sounds.keys()
                        .filter(|k| k.starts_with(&key_base_name))
                        .cloned()
                        .collect();
                    
                    for key_name in sounds_to_stop {
                        // 맵에서 먼저 제거
                        if let Some(audio) = self.active_sounds.remove(&key_name) {
                            let _ = audio.set_current_time(0.0);
                            let _ = audio.pause();
                            console::log_1(&format!("세트 {} 키 {} 소리 제거", set_idx, key_idx).into());
                        }
                    }
                }
                false
            },
            PianoMsg::StopSetSoundsIfReleased(set_idx) => {
                if set_idx < self.piano_sets.len() {
                    // 세트의 모든 키가 눌려있지 않고 서스테인이 꺼져 있을 때만 소리 정지
                    let all_keys_released = self.piano_sets[set_idx].iter()
                        .all(|&key_idx| !self.keys[key_idx].is_pressed);
                        
                    // 활성화된 세트인지 확인
                    let is_active_set = self.active_set == Some(set_idx);
                    
                    // 활성화된 세트는 소리를 정지하지 않음
                    if all_keys_released && !self.sustain && !is_active_set {
                        // 모든 키의 소리 정지
                        for &key_idx in &self.piano_sets[set_idx] {
                            let key_base_name = self.keys[key_idx].full_name();
                            
                            // 해당 키에 관련된 모든 소리 찾기 (타임스탬프 무관)
                            let sounds_to_stop: Vec<String> = self.active_sounds.keys()
                                .filter(|k| k.starts_with(&key_base_name))
                                .cloned()
                                .collect();
                            
                            for key_name in sounds_to_stop {
                                // 맵에서 먼저 제거
                                if let Some(audio) = self.active_sounds.remove(&key_name) {
                                    let _ = audio.set_current_time(0.0);
                                    let _ = audio.pause();
                                    console::log_1(&format!("세트 키 {} 소리 정지", key_base_name).into());
                                }
                            }
                        }
                    } else {
                        console::log_1(&format!("세트 {} 소리 정지 취소 (키가 다시 눌려있거나 서스테인 활성화됨 또는 활성 세트임)", set_idx).into());
                    }
                }
                false
            },
            PianoMsg::StopSetKeySound(set_idx, key_idx) => {
                // 키가 눌려있지 않고 서스테인이 꺼져 있을 때만 소리 정지
                if set_idx < self.piano_sets.len() && key_idx < self.keys.len() {
                    // 활성화된 세트인지 확인
                    let is_active_set = self.active_set == Some(set_idx);
                    
                    if !self.keys[key_idx].is_pressed && !self.sustain && !is_active_set {
                        let key_base_name = self.keys[key_idx].full_name();
                            
                        // 해당 키에 관련된 모든 소리 찾기 (타임스탬프 무관)
                        let sounds_to_stop: Vec<String> = self.active_sounds.keys()
                            .filter(|k| k.starts_with(&key_base_name))
                            .cloned()
                            .collect();
                        
                        for key_name in sounds_to_stop {
                            // 맵에서 먼저 제거
                            if let Some(audio) = self.active_sounds.remove(&key_name) {
                                let _ = audio.set_current_time(0.0);
                                let _ = audio.pause();
                                console::log_1(&format!("세트 키 {} 소리 정지", key_base_name).into());
                            }
                        }
                    } else {
                        console::log_1(&format!("세트 키 {} 소리 정지 취소 (키가 다시 눌려있거나 서스테인 활성화됨 또는 활성 세트임)", self.keys[key_idx].full_name()).into());
                    }
                }
                false
            },
            PianoMsg::AddActiveSound(key_name, audio) => {
                // active_sounds에 오디오 요소 추가
                self.active_sounds.insert(key_name, audio);
                false
            },
            PianoMsg::RemoveActiveSound(key_name) => {
                // active_sounds에서 오디오 요소 제거
                self.active_sounds.remove(&key_name);
                false
            },
            PianoMsg::FadeOutSound(key_name, current_volume) => {
                if let Some(audio) = self.active_sounds.get(&key_name) {
                    // 볼륨 단계적으로 줄이기 (페이드아웃 속도 더 빠르게 조정)
                    let new_volume = (current_volume - 0.1).max(0.0);
                    audio.set_volume(new_volume);
                    
                    // 볼륨이 0에 도달하지 않았으면 계속 페이드아웃
                    if new_volume > 0.0 {
                        let key_name_clone = key_name.clone();
                        let link = ctx.link().clone();
                        
                        // 페이드아웃 간격 더 짧게 조정 (30ms)
                        let timeout = Timeout::new(30, move || {
                            link.send_message(PianoMsg::FadeOutSound(key_name_clone, new_volume));
                        });
                        timeout.forget();
                    } else {
                        // 볼륨이 0에 도달하면 소리 정지
                        ctx.link().send_message(PianoMsg::StopSound(key_name));
                    }
                }
                false
            },
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render && self._keyboard_listeners.is_none() {
            // 첫 렌더링 시에만 키보드 이벤트 리스너 등록
            self.setup_keyboard_listeners(ctx);
        }
    }

    fn destroy(&mut self, _ctx: &Context<Self>) {
        // 컴포넌트가 파괴될 때 이벤트 리스너 정리
        if let Some((keydown_closure, keyup_closure, blur_closure, focus_out_closure, mouse_leave_closure, visibility_change_closure)) = &self._keyboard_listeners {
            let document = web_sys::window().unwrap().document().unwrap();
            let window = web_sys::window().unwrap();
            
            // 이벤트 리스너 제거
            let _ = document.remove_event_listener_with_callback(
                "keydown", 
                keydown_closure.as_ref().unchecked_ref()
            );
            let _ = document.remove_event_listener_with_callback(
                "keyup", 
                keyup_closure.as_ref().unchecked_ref()
            );
            let _ = window.remove_event_listener_with_callback(
                "blur", 
                blur_closure.as_ref().unchecked_ref()
            );
            let _ = document.remove_event_listener_with_callback(
                "focusout", 
                focus_out_closure.as_ref().unchecked_ref()
            );
            let _ = document.remove_event_listener_with_callback(
                "visibilitychange", 
                visibility_change_closure.as_ref().unchecked_ref()
            );
        }
        
        // 모든 활성 소리 정지
        for (_, audio) in &self.active_sounds {
            let _ = audio.pause();
        }
        self.active_sounds.clear();
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
                                        // 왼손과 오른손 옥타브 범위에 있는지 확인
                                        let left_start_note = NOTE_NAMES[self.left_hand_start_note_idx];
                                        let right_start_note = NOTE_NAMES[self.right_hand_start_note_idx];
                                        
                                        let is_left_hand = if key.name == left_start_note && key.octave == self.left_hand_octave {
                                            // 현재 옥타브 시작음
                                            true
                                        } else if key.name == left_start_note && key.octave == self.left_hand_octave + 1 {
                                            // 다음 옥타브 시작음
                                            true
                                        } else if key.octave == self.left_hand_octave && 
                                                NOTE_NAMES.iter().position(|&n| n == key.name).unwrap_or(0) > self.left_hand_start_note_idx {
                                            // 현재 옥타브의 시작음보다 높은 음
                                            true
                                        } else if key.octave == self.left_hand_octave + 1 && 
                                                NOTE_NAMES.iter().position(|&n| n == key.name).unwrap_or(0) < self.left_hand_start_note_idx {
                                            // 다음 옥타브의 시작음보다 낮은 음
                                            true
                                        } else {
                                            false
                                        };
                                            
                                        let is_right_hand = if key.name == right_start_note && key.octave == self.right_hand_octave {
                                            // 현재 옥타브 시작음
                                            true
                                        } else if key.name == right_start_note && key.octave == self.right_hand_octave + 1 {
                                            // 다음 옥타브 시작음
                                            true
                                        } else if key.octave == self.right_hand_octave && 
                                                NOTE_NAMES.iter().position(|&n| n == key.name).unwrap_or(0) > self.right_hand_start_note_idx {
                                            // 현재 옥타브의 시작음보다 높은 음
                                            true
                                        } else if key.octave == self.right_hand_octave + 1 && 
                                                NOTE_NAMES.iter().position(|&n| n == key.name).unwrap_or(0) < self.right_hand_start_note_idx {
                                            // 다음 옥타브의 시작음보다 낮은 음
                                            true
                                        } else {
                                            false
                                        };
                                        
                                        // 클래스 이름 계산
                                        let mut class_names = classes!("piano-key", "white-key");
                                        if key.is_pressed {
                                            class_names.push("pressed");
                                        }
                                        // 키보드 입력이 활성화된 경우에만 손 영역 표시
                                        if self.keyboard_input_enabled {
                                            if is_left_hand {
                                                class_names.push("left-hand-range");
                                            }
                                            if is_right_hand {
                                                class_names.push("right-hand-range");
                                            }
                                        }
                                        
                                        html! {
                                            <div 
                                                class={class_names}
                                                onmousedown={
                                                    let i = *index;
                                                    if self.set_edit_mode && self.current_edit_set.is_some() {
                                                        ctx.link().callback(move |_| PianoMsg::ToggleKeyInSetWithSound(i))
                                                    } else {
                                                        ctx.link().callback(move |_| PianoMsg::KeyPressed(i))
                                                    }
                                                }
                                                onmouseup={
                                                    let i = *index;
                                                    if self.set_edit_mode && self.current_edit_set.is_some() {
                                                        ctx.link().callback(move |_| PianoMsg::KeyReleased(i))
                                                    } else {
                                                        ctx.link().callback(move |_| PianoMsg::KeyReleased(i))
                                                    }
                                                }
                                                onmouseleave={
                                                    let i = *index;
                                                    if self.set_edit_mode && self.current_edit_set.is_some() {
                                                        ctx.link().callback(move |_| PianoMsg::KeyReleased(i))
                                                    } else {
                                                        ctx.link().callback(move |_| PianoMsg::KeyReleased(i))
                                                    }
                                                }
                                                onmouseout={
                                                    let i = *index;
                                                    if self.set_edit_mode && self.current_edit_set.is_some() {
                                                        ctx.link().callback(move |_| PianoMsg::KeyReleased(i))
                                                    } else {
                                                        ctx.link().callback(move |_| PianoMsg::KeyReleased(i))
                                                    }
                                                }
                                                ontouchstart={
                                                    let i = *index;
                                                    if self.set_edit_mode && self.current_edit_set.is_some() {
                                                        ctx.link().callback(move |_| PianoMsg::ToggleKeyInSetWithSound(i))
                                                    } else {
                                                        ctx.link().callback(move |_| PianoMsg::KeyPressed(i))
                                                    }
                                                }
                                                ontouchend={
                                                    let i = *index;
                                                    if self.set_edit_mode && self.current_edit_set.is_some() {
                                                        ctx.link().callback(move |_| PianoMsg::KeyReleased(i))
                                                    } else {
                                                        ctx.link().callback(move |_| PianoMsg::KeyReleased(i))
                                                    }
                                                }
                                                ontouchcancel={
                                                    let i = *index;
                                                    if self.set_edit_mode && self.current_edit_set.is_some() {
                                                        ctx.link().callback(move |_| PianoMsg::KeyReleased(i))
                                                    } else {
                                                        ctx.link().callback(move |_| PianoMsg::KeyReleased(i))
                                                    }
                                                }
                                                title={key.full_name()}
                                                style="flex: 1;"
                                            >
                                                <span class="key-label">{key.full_name()}</span>
                                                {
                                                    // 키보드 입력이 활성화된 경우 키보드 키 표시
                                                    if self.keyboard_input_enabled {
                                                        if let Some(keyboard_key) = self.find_keyboard_key_for_piano(key) {
                                                            html! {
                                                                <span class="keyboard-key-label">{keyboard_key}</span>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                }
                                                {
                                                    // 세트 수정 모드에서 현재 세트에 포함된 키인지 표시
                                                    if self.set_edit_mode {
                                                        if let Some(set_idx) = self.current_edit_set {
                                                            if self.piano_sets[set_idx].contains(&i) {
                                                                html! {
                                                                    <span class="set-marker">{"✓"}</span>
                                                                }
                                                            } else {
                                                                html! {}
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                }
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
                                        
                                        // 왼손과 오른손 옥타브 범위에 있는지 확인 (검은 건반용)
                                        let note_idx = NOTE_NAMES.iter().position(|&n| n == key.name).unwrap_or(0);
                                        
                                        let left_start_note = NOTE_NAMES[self.left_hand_start_note_idx];
                                        let right_start_note = NOTE_NAMES[self.right_hand_start_note_idx];
                                        
                                        let is_left_hand = if key.name == left_start_note && key.octave == self.left_hand_octave {
                                            // 현재 옥타브 시작음
                                            true
                                        } else if key.name == left_start_note && key.octave == self.left_hand_octave + 1 {
                                            // 다음 옥타브 시작음
                                            true
                                        } else if key.octave == self.left_hand_octave && 
                                                note_idx > self.left_hand_start_note_idx {
                                            // 현재 옥타브의 시작음보다 높은 음
                                            true
                                        } else if key.octave == self.left_hand_octave + 1 && 
                                                note_idx < self.left_hand_start_note_idx {
                                            // 다음 옥타브의 시작음보다 낮은 음
                                            true
                                        } else {
                                            false
                                        };
                                            
                                        let is_right_hand = if key.name == right_start_note && key.octave == self.right_hand_octave {
                                            // 현재 옥타브 시작음
                                            true
                                        } else if key.name == right_start_note && key.octave == self.right_hand_octave + 1 {
                                            // 다음 옥타브 시작음
                                            true
                                        } else if key.octave == self.right_hand_octave && 
                                                note_idx > self.right_hand_start_note_idx {
                                            // 현재 옥타브의 시작음보다 높은 음
                                            true
                                        } else if key.octave == self.right_hand_octave + 1 && 
                                                note_idx < self.right_hand_start_note_idx {
                                            // 다음 옥타브의 시작음보다 낮은 음
                                            true
                                        } else {
                                            false
                                        };
                                        
                                        // 클래스 이름 계산
                                        let mut class_names = classes!("piano-key", "black-key");
                                        if key.is_pressed {
                                            class_names.push("pressed");
                                        }
                                        // 키보드 입력이 활성화된 경우에만 손 영역 표시
                                        if self.keyboard_input_enabled {
                                            if is_left_hand {
                                                class_names.push("left-hand-range");
                                            }
                                            if is_right_hand {
                                                class_names.push("right-hand-range");
                                            }
                                        }
                                        
                                        html! {
                                            <div 
                                                class={class_names}
                                                style={format!("top: 0; left: {}%", position)}
                                                onmousedown={
                                                    let i = *index;
                                                    if self.set_edit_mode && self.current_edit_set.is_some() {
                                                        ctx.link().callback(move |_| PianoMsg::ToggleKeyInSetWithSound(i))
                                                    } else {
                                                        ctx.link().callback(move |_| PianoMsg::KeyPressed(i))
                                                    }
                                                }
                                                onmouseup={
                                                    let i = *index;
                                                    if self.set_edit_mode && self.current_edit_set.is_some() {
                                                        ctx.link().callback(move |_| PianoMsg::KeyReleased(i))
                                                    } else {
                                                        ctx.link().callback(move |_| PianoMsg::KeyReleased(i))
                                                    }
                                                }
                                                onmouseleave={
                                                    let i = *index;
                                                    if self.set_edit_mode && self.current_edit_set.is_some() {
                                                        ctx.link().callback(move |_| PianoMsg::KeyReleased(i))
                                                    } else {
                                                        ctx.link().callback(move |_| PianoMsg::KeyReleased(i))
                                                    }
                                                }
                                                onmouseout={
                                                    let i = *index;
                                                    if self.set_edit_mode && self.current_edit_set.is_some() {
                                                        ctx.link().callback(move |_| PianoMsg::KeyReleased(i))
                                                    } else {
                                                        ctx.link().callback(move |_| PianoMsg::KeyReleased(i))
                                                    }
                                                }
                                                ontouchstart={
                                                    let i = *index;
                                                    if self.set_edit_mode && self.current_edit_set.is_some() {
                                                        ctx.link().callback(move |_| PianoMsg::ToggleKeyInSetWithSound(i))
                                                    } else {
                                                        ctx.link().callback(move |_| PianoMsg::KeyPressed(i))
                                                    }
                                                }
                                                ontouchend={
                                                    let i = *index;
                                                    if self.set_edit_mode && self.current_edit_set.is_some() {
                                                        ctx.link().callback(move |_| PianoMsg::KeyReleased(i))
                                                    } else {
                                                        ctx.link().callback(move |_| PianoMsg::KeyReleased(i))
                                                    }
                                                }
                                                ontouchcancel={
                                                    let i = *index;
                                                    if self.set_edit_mode && self.current_edit_set.is_some() {
                                                        ctx.link().callback(move |_| PianoMsg::KeyReleased(i))
                                                    } else {
                                                        ctx.link().callback(move |_| PianoMsg::KeyReleased(i))
                                                    }
                                                }
                                                title={key.full_name()}
                                            >
                                                <span class="key-label">{key.full_name()}</span>
                                                {
                                                    // 키보드 입력이 활성화된 경우 키보드 키 표시
                                                    if self.keyboard_input_enabled {
                                                        if let Some(keyboard_key) = self.find_keyboard_key_for_piano(key) {
                                                            html! {
                                                                <span class="keyboard-key-label black">{keyboard_key}</span>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                }
                                                {
                                                    // 세트 수정 모드에서 현재 세트에 포함된 키인지 표시
                                                    if self.set_edit_mode {
                                                        if let Some(set_idx) = self.current_edit_set {
                                                            if self.piano_sets[set_idx].contains(&i) {
                                                                html! {
                                                                    <span class="set-marker black">{"✓"}</span>
                                                                }
                                                            } else {
                                                                html! {}
                                                            }
                                                        } else {
                                                            html! {}
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
                        </div>
                    </div>
                    <div class="settings-section">
                        <div class="settings-container">
                            <div class="settings-row">
                                <div class="octave-control">
                                    <button 
                                        onclick={ctx.link().callback(|_| PianoMsg::ScrollPiano(-1))}
                                        disabled={self.start_octave == 0}
                                        title="옥타브 아래로 (/ 키)"
                                    >
                                        {"◀"}
                                    </button>
                                    <div class="octave-display">{format!("옥타브: {}-{}", self.start_octave, end_octave)}</div>
                                    <button 
                                        onclick={ctx.link().callback(|_| PianoMsg::ScrollPiano(1))}
                                        disabled={self.start_octave >= 4}
                                        title="옥타브 위로 (] 키)"
                                    >
                                        {"▶"}
                                    </button>
                                </div>
                                <div class="sustain-control">
                                    <button 
                                        class={classes!("sustain-button", if self.sustain { "active" } else { "" })}
                                        onclick={ctx.link().callback(|_| PianoMsg::ToggleSustain)}
                                        title={if self.sustain { "서스테인 끄기 (스페이스바)" } else { "서스테인 켜기 (스페이스바)" }}
                                    >
                                        {"서스테인"}
                                    </button>
                                </div>
                            </div>
                            
                            <div class="settings-row keyboard-settings">
                                <div class="keyboard-octave-display">
                                    <div class="octave-info">
                                        <div class="octave-label left">
                                            <span class={classes!("octave-value", if !self.keyboard_input_enabled { "disabled" } else { "" })}>
                                                {NOTE_NAMES[self.left_hand_start_note_idx]}{self.left_hand_octave}{"-"}{NOTE_NAMES[self.left_hand_start_note_idx]}{self.left_hand_octave+1}
                                            </span>
                                        </div>
                                        <div class="keyboard-toggle">
                                            <button 
                                                class={classes!("keyboard-toggle-button", if self.keyboard_input_enabled { "active" } else { "" })}
                                                onclick={ctx.link().callback(|_| PianoMsg::ToggleKeyboardInput)}
                                                title={if self.keyboard_input_enabled { "키보드 입력 비활성화" } else { "키보드 입력 활성화" }}
                                            >
                                                {if self.keyboard_input_enabled { "⌨️ ON" } else { "⌨️ OFF" }}
                                            </button>
                                        </div>
                                        <div class="octave-label right">
                                            <span class={classes!("octave-value", if !self.keyboard_input_enabled { "disabled" } else { "" })}>
                                                {NOTE_NAMES[self.right_hand_start_note_idx]}{self.right_hand_octave}{"-"}{NOTE_NAMES[self.right_hand_start_note_idx]}{self.right_hand_octave+1}
                                            </span>
                                        </div>
                                    </div>
                                </div>
                            </div>
                            
                            <div class="settings-row piano-sets">
                                <div class="piano-sets-layout">
                                    <button 
                                        class={classes!("edit-mode-button", if self.set_edit_mode { "active" } else { "" })}
                                        onclick={ctx.link().callback(|_| PianoMsg::ToggleSetEditMode)}
                                        title={if self.set_edit_mode { "수정 모드 비활성화" } else { "수정 모드 활성화" }}
                                    >
                                        {if self.set_edit_mode { "✏️" } else { "✏️" }}
                                    </button>
                                    <button 
                                        class="edit-mode-button"
                                        onclick={ctx.link().callback(|_| PianoMsg::ClearAllSets)}
                                        title="모든 세트 초기화 (~ 키)"
                                    >
                                        {"🗑️"}
                                    </button>
                                    <div class="piano-sets-buttons">
                                        {
                                            // 세트 버튼 생성 (0-9)
                                            (0..10).map(|set_idx| {
                                                let set_label = if set_idx == 9 { "0".to_string() } else { (set_idx + 1).to_string() };
                                                let has_notes = !self.piano_sets[set_idx].is_empty();
                                                let is_selected = self.current_edit_set == Some(set_idx);
                                                
                                                html! {
                                                    <button 
                                                        class={classes!(
                                                            "set-button", 
                                                            if has_notes { "has-notes" } else { "" },
                                                            if is_selected { "selected" } else { "" }
                                                        )}
                                                        onmousedown={ctx.link().callback(move |_| PianoMsg::PlaySet(set_idx))}
                                                        onmouseup={ctx.link().callback(move |_| PianoMsg::ReleaseSet(set_idx))}
                                                        onmouseleave={ctx.link().callback(move |_| PianoMsg::ReleaseSet(set_idx))}
                                                    >
                                                        {set_label}
                                                    </button>
                                                }
                                            }).collect::<Html>()
                                        }
                                    </div>
                                </div>
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
        let key_name = self.keys[key_idx].full_name();
        if let Some(audio) = self.active_sounds.remove(&key_name) {
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
        let key_name = self.keys[key_idx].full_name();
        if let Some(audio) = self.active_sounds.remove(&key_name) {
            let _ = audio.pause();
            let _ = audio.set_src("");  // 리소스 해제
            
            console::log_1(&format!("피아노 노트 중지: {}", key_name).into());
        }
    }

    // 키보드 키에 매핑된 피아노 키 인덱스 찾기
    fn find_piano_key_by_keyboard(&self, keyboard_key: &str) -> Option<usize> {
        // 매핑 정보 찾기
        let mapping = self.key_mappings.iter().find(|m| m.keyboard_key == keyboard_key)?;
        
        // 왼손/오른손에 따라 옥타브 결정
        let octave = if mapping.is_left_hand {
            self.left_hand_octave + mapping.octave_offset
        } else {
            self.right_hand_octave + mapping.octave_offset
        };
        
        // 해당 노트와 옥타브를 가진 피아노 키 찾기
        for (idx, key) in self.keys.iter().enumerate() {
            if key.name == mapping.piano_note && key.octave == octave {
                return Some(idx);
            }
        }
        
        None
    }

    // 키 매핑 생성 헬퍼 함수
    fn create_key_mappings(
        key_mappings: &mut Vec<KeyMapping>, 
        pressed_keyboard_keys: &mut HashMap<String, bool>,
        left_hand_keys: &[&str], 
        right_hand_keys: &[&str],
        left_start_note_idx: usize,
        right_start_note_idx: usize
    ) {
        key_mappings.clear();
        
        // 왼손 키 매핑
        for (i, key) in left_hand_keys.iter().enumerate() {
            let note_idx = (left_start_note_idx + i) % 12;
            let octave_offset = ((left_start_note_idx + i) / 12) as i32;
            
            let keyboard_key = key.to_string();
            let piano_note = NOTE_NAMES[note_idx].to_string();
            
            key_mappings.push(KeyMapping {
                keyboard_key: keyboard_key.clone(),
                piano_note,
                is_left_hand: true,
                octave_offset,
            });
            
            pressed_keyboard_keys.insert(keyboard_key, false);
        }
        
        // 오른손 키 매핑
        for (i, key) in right_hand_keys.iter().enumerate() {
            let note_idx = (right_start_note_idx + i) % 12;
            let octave_offset = ((right_start_note_idx + i) / 12) as i32;
            
            let keyboard_key = key.to_string();
            let piano_note = NOTE_NAMES[note_idx].to_string();
            
            key_mappings.push(KeyMapping {
                keyboard_key: keyboard_key.clone(),
                piano_note,
                is_left_hand: false,
                octave_offset,
            });
            
            pressed_keyboard_keys.insert(keyboard_key, false);
        }
    }
    
    // 키 매핑 재생성
    fn recreate_key_mappings(&mut self) {
        let left_hand_keys = ["z", "x", "c", "v", "a", "s", "d", "f", "w", "e", "r", "t", "y"];
        let right_hand_keys = ["m", ",", ".", "/", "j", "k", "l", ";", "u", "i", "o", "p", "["];
        
        Self::create_key_mappings(
            &mut self.key_mappings, 
            &mut self.pressed_keyboard_keys, 
            &left_hand_keys, 
            &right_hand_keys, 
            self.left_hand_start_note_idx, 
            self.right_hand_start_note_idx
        );
    }

    // 특정 옥타브의 눌린 키를 모두 해제
    fn release_keys_in_octave(&mut self, ctx: &Context<Self>, octave: i32, is_left_hand: bool) {
        let keys_to_release: Vec<usize> = self.keys.iter().enumerate()
            .filter(|(_, key)| {
                key.is_pressed && key.octave == octave &&
                // 왼손 또는 오른손 영역만 처리
                self.is_key_in_hand_range(key, octave, is_left_hand)
            })
            .map(|(idx, _)| idx)
            .collect();
            
        for idx in keys_to_release {
            let _ = yew::Component::update(self, ctx, PianoMsg::KeyReleased(idx));
        }
    }
    
    // 노트 인덱스가 변경되었을 때 영역에서 벗어난 키를 해제
    fn release_keys_for_changed_note_idx(&mut self, ctx: &Context<Self>, old_idx: usize, new_idx: usize, octave: i32, is_left_hand: bool) {
        let keys_to_release: Vec<usize> = self.keys.iter().enumerate()
            .filter(|(_, key)| {
                if !key.is_pressed {
                    return false;
                }
                
                let note_idx = NOTE_NAMES.iter().position(|&n| n == key.name.replace("#", "")).unwrap_or(0);
                
                // 이전 영역에 있었지만 새 영역에는 없는 키 찾기
                let was_in_range = self.is_note_in_hand_range(key.octave, note_idx, octave, old_idx);
                let is_in_range = self.is_note_in_hand_range(key.octave, note_idx, octave, new_idx);
                
                was_in_range && !is_in_range && 
                // 왼손 또는 오른손 영역만 처리
                is_left_hand == (key.octave == self.left_hand_octave || key.octave == self.left_hand_octave + 1)
            })
            .map(|(idx, _)| idx)
            .collect();
            
        for idx in keys_to_release {
            let _ = yew::Component::update(self, ctx, PianoMsg::KeyReleased(idx));
        }
    }
    
    // 키가 특정 손의 영역에 속하는지 확인
    fn is_key_in_hand_range(&self, key: &PianoKey, octave: i32, is_left_hand: bool) -> bool {
        let note_idx = NOTE_NAMES.iter().position(|&n| n == key.name.replace("#", "")).unwrap_or(0);
        let check_idx = if is_left_hand { self.left_hand_start_note_idx } else { self.right_hand_start_note_idx };
        
        self.is_note_in_hand_range(key.octave, note_idx, octave, check_idx)
    }
    
    // 노트가 손의 영역에 속하는지 확인
    fn is_note_in_hand_range(&self, key_octave: i32, note_idx: usize, hand_octave: i32, hand_start_idx: usize) -> bool {
        if key_octave == hand_octave {
            // 현재 옥타브에서는 시작 인덱스보다 높은 노트만
            note_idx >= hand_start_idx
        } else if key_octave == hand_octave + 1 {
            // 다음 옥타브에서는 시작 인덱스보다 낮은 노트만
            note_idx < hand_start_idx
        } else {
            false
        }
    }

    // 피아노 키에 매핑된 키보드 키 찾기
    fn find_keyboard_key_for_piano(&self, piano_key: &PianoKey) -> Option<String> {
        // 매핑 정보 찾기
        for mapping in &self.key_mappings {
            // 왼손/오른손에 따라 옥타브 결정
            let octave = if mapping.is_left_hand {
                self.left_hand_octave + mapping.octave_offset
            } else {
                self.right_hand_octave + mapping.octave_offset
            };
            
            // 해당 노트와 옥타브를 가진 매핑 찾기
            if mapping.piano_note == piano_key.name && octave == piano_key.octave {
                // 스페이스바인 경우 "Space"로 표시
                if mapping.keyboard_key == " " {
                    return Some("Space".to_string());
                }
                return Some(mapping.keyboard_key.clone());
            }
        }
        
        None
    }
    
    // 특정 세트의 모든 소리 정지
    fn stop_set_sounds(&mut self, set_idx: usize) {
        if set_idx < self.piano_sets.len() {
            // 먼저 세트의 모든 키가 눌려있지 않은지 다시 확인
            let all_keys_released = self.piano_sets[set_idx].iter()
                .all(|&key_idx| !self.keys[key_idx].is_pressed);
                
            if all_keys_released {
                // 각 키의 소리 정지
                for &key_idx in &self.piano_sets[set_idx] {
                    let key_base_name = self.keys[key_idx].full_name();
                    
                    // 해당 키에 관련된 모든 소리 찾기 (타임스탬프 무관)
                    let sounds_to_stop: Vec<String> = self.active_sounds.keys()
                        .filter(|k| k.starts_with(&key_base_name))
                        .cloned()
                        .collect();
                    
                    for key_name in sounds_to_stop {
                        // 맵에서 먼저 제거
                        if let Some(audio) = self.active_sounds.remove(&key_name) {
                            let _ = audio.set_current_time(0.0);
                            let _ = audio.pause();
                        }
                    }
                }
                
                // 로그 출력
                console::log_1(&format!("세트 {} 소리 모두 정지", set_idx).into());
            } else {
                console::log_1(&format!("세트 {} 소리 정지 취소 (키가 다시 눌려있음)", set_idx).into());
            }
        }
    }
    
    // 특정 세트의 모든 소리 정리 (재생 유지하며 기존 소리만 제거)
    fn clean_set_sounds(&mut self, set_idx: usize) {
        if set_idx < self.piano_sets.len() {
            // 각 키의 이전 소리만 제거 (현재 재생 중인 것은 유지)
            for &key_idx in &self.piano_sets[set_idx] {
                let key_base_name = self.keys[key_idx].full_name();
                
                // 해당 키에 관련된 모든 소리 찾기 (타임스탬프 무관)
                let sounds_to_clean: Vec<String> = self.active_sounds.keys()
                    .filter(|k| k.starts_with(&key_base_name))
                    .cloned()
                    .collect();
                
                for key_name in sounds_to_clean {
                    // HashMap에서만 제거하고 pause 호출하지 않음
                    if let Some(_) = self.active_sounds.remove(&key_name) {
                        console::log_1(&format!("세트 {} 키 {} 이전 소리 정리", set_idx, key_idx).into());
                    }
                }
            }
            console::log_1(&format!("세트 {} 소리 정리 완료", set_idx).into());
        }
    }

    // 키보드 이벤트 리스너 설정
    fn setup_keyboard_listeners(&mut self, ctx: &Context<Self>) {
        // 첫 렌더링 시 키보드 이벤트 리스너 등록
        let document = web_sys::window().unwrap().document().unwrap();
        
        // 키 다운 이벤트 핸들러
        let link_down = ctx.link().clone();
        let keydown_callback = Closure::wrap(Box::new(move |event: KeyboardEvent| {
            let key = event.key();
            
            // 기능키(F1-F12)와 특수 키 조합(Ctrl+R, Ctrl+Shift+I 등)은 브라우저 기본 동작 허용
            if key.starts_with("F") || event.ctrl_key() || event.alt_key() || event.meta_key() {
                // 피아노 앱에서 처리하지 않는 기능키는 기본 동작 유지
                console::log_1(&format!("브라우저 기능키 감지: {}", key).into());
                // 단, 피아노 앱에서 사용하는 키는 처리
                link_down.send_message(PianoMsg::KeyboardKeyDown(key));
                return;
            }
            
            // 그 외 일반 키는 기본 동작 방지(페이지 스크롤 등)
            event.prevent_default();
            event.stop_propagation();
            
            console::log_1(&format!("Key down: {}", key).into());
            
            // 세트 키(1-9, 0)인 경우 
            let is_set_key = matches!(key.as_str(), "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "0");
            
            if is_set_key && !event.repeat() {
                let set_idx = if key == "0" { 9 } else { key.parse::<usize>().unwrap_or(0) - 1 };
                console::log_1(&format!("세트 키 감지: 세트 {}", set_idx).into());
                
                // 먼저 KeyboardKeyDown 메시지를 보내 키 상태 업데이트
                link_down.send_message(PianoMsg::KeyboardKeyDown(key.clone()));
                
                // 세트 재생 메시지 전송 (마우스 로직과 동일하게 처리)
                link_down.send_message(PianoMsg::PlaySet(set_idx));
            } else {
                // 일반 키보드 처리는 기존대로
                link_down.send_message(PianoMsg::KeyboardKeyDown(key));
            }
            
            // 세트 키가 아닌 경우에만 즉시 상태 업데이트 요청
            if !is_set_key {
                // 강제로 키 상태 업데이트 요청
                let link = link_down.clone();
                let timeout = Timeout::new(10, move || {
                    link.send_message(PianoMsg::ForceKeyUpdate);
                });
                timeout.forget();
            }
        }) as Box<dyn FnMut(KeyboardEvent)>);
        
        // 키 업 이벤트 핸들러
        let link_up = ctx.link().clone();
        let keyup_callback = Closure::wrap(Box::new(move |event: KeyboardEvent| {
            let key = event.key();
            
            // 기능키(F1-F12)와 특수 키 조합(Ctrl+R, Ctrl+Shift+I 등)은 브라우저 기본 동작 허용
            if key.starts_with("F") || event.ctrl_key() || event.alt_key() || event.meta_key() {
                // 피아노 앱에서 처리하지 않는 기능키는 기본 동작 유지
                console::log_1(&format!("브라우저 기능키 감지(키업): {}", key).into());
                // 단, 피아노 앱에서 사용하는 키는 처리
                link_up.send_message(PianoMsg::KeyboardKeyUp(key));
                return;
            }
            
            // 그 외 일반 키는 기본 동작 방지
            event.prevent_default();
            event.stop_propagation();
            
            console::log_1(&format!("Key up: {}", key).into());
            
            // 세트 키(1-9, 0)인 경우
            let is_set_key = matches!(key.as_str(), "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "0");
            
            if is_set_key {
                let set_idx = if key == "0" { 9 } else { key.parse::<usize>().unwrap_or(0) - 1 };
                console::log_1(&format!("세트 키 떼기: 세트 {}", set_idx).into());
                
                // 먼저 KeyboardKeyUp 메시지를 보내 키 상태 업데이트
                link_up.send_message(PianoMsg::KeyboardKeyUp(key.clone()));
                
                // 세트 해제 메시지 전송 (마우스 로직과 동일하게 처리)
                link_up.send_message(PianoMsg::ReleaseSet(set_idx));
            } else {
                // 일반 키보드 처리는 기존대로
                link_up.send_message(PianoMsg::KeyboardKeyUp(key));
            }
            
            // 상태 업데이트 요청
            let link = link_up.clone();
            let timeout = Timeout::new(10, move || {
                link.send_message(PianoMsg::ForceKeyUpdate);
            });
            timeout.forget();
            
            // 조금 더 지연된 두 번째 업데이트 요청
            let link2 = link_up.clone();
            let timeout2 = Timeout::new(100, move || {
                link2.send_message(PianoMsg::ForceKeyUpdate);
            });
            timeout2.forget();
        }) as Box<dyn FnMut(KeyboardEvent)>);
        
        // 포커스/블러 이벤트 핸들러 추가
        let link_blur = ctx.link().clone();
        let blur_callback = Closure::wrap(Box::new(move |_event| {
            // 윈도우가 포커스를 잃으면 모든 키 리셋
            link_blur.send_message(PianoMsg::ResetAllKeys);
        }) as Box<dyn FnMut(web_sys::Event)>);
        
        // 포커스 아웃 이벤트 핸들러 추가
        let link_focus_out = ctx.link().clone();
        let focus_out_callback = Closure::wrap(Box::new(move |_event| {
            // 윈도우가 포커스를 잃으면 모든 키 리셋
            link_focus_out.send_message(PianoMsg::ResetAllKeys);
        }) as Box<dyn FnMut(web_sys::FocusEvent)>);

        // 이벤트 리스너 등록
        document.add_event_listener_with_callback("keydown", keydown_callback.as_ref().unchecked_ref())
            .expect("이벤트 리스너 등록 실패");
        document.add_event_listener_with_callback("keyup", keyup_callback.as_ref().unchecked_ref())
            .expect("이벤트 리스너 등록 실패");
            
        // 윈도우 블러 이벤트 리스너 등록
        let window = web_sys::window().unwrap();
        window.add_event_listener_with_callback("blur", blur_callback.as_ref().unchecked_ref())
            .expect("블러 이벤트 리스너 등록 실패");
        document.add_event_listener_with_callback("focusout", focus_out_callback.as_ref().unchecked_ref())
            .expect("포커스 아웃 이벤트 리스너 등록 실패");
            
        // 마우스가 영역을 벗어났을 때 키 리셋을 위한 이벤트 핸들러
        let link_mouse_leave = ctx.link().clone();
        let mouse_leave_callback = Closure::wrap(Box::new(move |_event| {
            link_mouse_leave.send_message(PianoMsg::ResetAllKeys);
        }) as Box<dyn FnMut(web_sys::MouseEvent)>);
        
        // 페이지 가시성 변경 이벤트 핸들러
        let link_visibility = ctx.link().clone();
        let visibility_callback = Closure::wrap(Box::new(move |_event| {
            if let Some(document) = web_sys::window().unwrap().document() {
                if document.hidden() {
                    link_visibility.send_message(PianoMsg::ResetAllKeys);
                }
            }
        }) as Box<dyn FnMut(web_sys::Event)>);
        
        document.add_event_listener_with_callback("visibilitychange", visibility_callback.as_ref().unchecked_ref())
            .expect("가시성 변경 이벤트 리스너 등록 실패");
        
        // 리스너 보관 (메모리 누수 방지)
        self._keyboard_listeners = Some((keydown_callback, keyup_callback, blur_callback, focus_out_callback, mouse_leave_callback, visibility_callback));
        
        // 이미 이벤트가 발생한 것처럼 처리해서 초기화
        ctx.link().send_message(PianoMsg::ResetAllKeys);

        // 주기적 상태 체크 타이머 추가
        let link_timer = ctx.link().clone();
        let timeout = Timeout::new(1000, move || {
            link_timer.send_message(PianoMsg::ForceKeyUpdate);
        });
        timeout.forget();
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