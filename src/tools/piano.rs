use wasm_bindgen::prelude::*;
use web_sys::{AudioContext, AudioNode, AudioParam, GainNode, HtmlAudioElement, KeyboardEvent, Document};
use yew::prelude::*;
use std::collections::HashMap;
use std::rc::Rc;
use gloo_timers::callback::Timeout;
use wasm_bindgen::closure::Closure;
use web_sys::console;
use js_sys;

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
    _keyboard_listener: Option<(Closure<dyn FnMut(KeyboardEvent)>, Closure<dyn FnMut(KeyboardEvent)>)>, // 키보드 이벤트 리스너
    keyboard_input_enabled: bool,   // 키보드 입력 활성화 여부
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
        let left_hand_keys = ["z", "x", "c", "v", "a", "s", "d", "f", "q", "w", "e", "r", "t"];
        
        // 오른손 키 매핑 (C4-C5 기본 옥타브)
        let right_hand_keys = ["n", "m", ",", ".", "j", "k", "l", ";", "u", "i", "o", "p", "["];
        
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
        pressed_keyboard_keys.insert("h".to_string(), false); // 오른손 옥타브 내림
        pressed_keyboard_keys.insert("y".to_string(), false); // 오른손 옥타브 올림
        pressed_keyboard_keys.insert("/".to_string(), false); // UI 범위 한 옥타브 아래로
        pressed_keyboard_keys.insert("]".to_string(), false); // UI 범위 한 옥타브 위로
        pressed_keyboard_keys.insert(" ".to_string(), false); // 스페이스바 (서스테인)
        pressed_keyboard_keys.insert("-".to_string(), false); // - (시작 음 낮추기)
        pressed_keyboard_keys.insert("=".to_string(), false); // + (시작 음 높이기)
        pressed_keyboard_keys.insert("0".to_string(), false); // 0 (매핑 초기화)

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
            _keyboard_listener: None,
            keyboard_input_enabled: false,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            PianoMsg::KeyPressed(index) => {
                if index < self.keys.len() {
                    self.keys[index].is_pressed = true;
                    
                    // 기존 소리를 중지하지 않고 새로운 오디오 요소 생성
                    let audio = HtmlAudioElement::new_with_src(&self.keys[index].audio_path())
                        .expect("오디오 요소 생성 실패");
                    
                    // 볼륨 설정
                    audio.set_volume(0.7);
                    
                    // 시작 위치 리셋
                    audio.set_current_time(0.0);
                    
                    // 오디오 요소 미리 로드
                    let _ = audio.load();
                    
                    // 기존 키 이름과 다른 고유 ID 생성 (타임스탬프 추가)
                    let key_name = format!("{}_{}", self.keys[index].full_name(), js_sys::Date::now());
                    
                    match audio.play() {
                        Ok(_) => {
                            console::log_1(&format!("피아노 노트 재생: {}", key_name).into());
                            self.active_sounds.insert(key_name, audio);
                        },
                        Err(err) => {
                            console::error_1(&format!("오디오 재생 실패: {:?}", err).into());
                        }
                    }
                    
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
            },
            PianoMsg::KeyboardKeyDown(key) => {
                // 키보드 입력이 비활성화된 경우 무시
                if !self.keyboard_input_enabled {
                    return false;
                }
                
                // 옥타브 변경 키 처리
                match key.as_str() {
                    "b" => return yew::Component::update(self, ctx, PianoMsg::ChangeLeftHandOctave(-1)),
                    "g" => return yew::Component::update(self, ctx, PianoMsg::ChangeLeftHandOctave(1)),
                    "h" => return yew::Component::update(self, ctx, PianoMsg::ChangeRightHandOctave(-1)),
                    "y" => return yew::Component::update(self, ctx, PianoMsg::ChangeRightHandOctave(1)),
                    "/" => return yew::Component::update(self, ctx, PianoMsg::MovePianoUIRange(-1)), // UI 범위를 한 옥타브 아래로
                    "]" => return yew::Component::update(self, ctx, PianoMsg::MovePianoUIRange(1)),  // UI 범위를 한 옥타브 위로
                    " " => {
                        // 스페이스바를 누르면 서스테인 활성화
                        if !self.sustain {
                            return yew::Component::update(self, ctx, PianoMsg::ToggleSustain);
                        }
                        return false;
                    },
                    "-" => {
                        // 왼손 시작 음 높이기
                        return yew::Component::update(self, ctx, PianoMsg::ChangeLeftHandStartNote(1));
                    },
                    "=" => {
                        // 오른손 시작 음 높이기
                        return yew::Component::update(self, ctx, PianoMsg::ChangeRightHandStartNote(1));
                    },
                    "Escape" => {
                        // Escape 키를 누르면 모든 키 리셋
                        return yew::Component::update(self, ctx, PianoMsg::ResetAllKeys);
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
                        return yew::Component::update(self, ctx, PianoMsg::KeyPressed(piano_key_idx));
                    }
                }
                false
            },
            PianoMsg::KeyboardKeyUp(key) => {
                // 키보드 입력이 비활성화된 경우 무시
                if !self.keyboard_input_enabled {
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
                    "b" | "g" | "h" | "y" | "/" | "]" | "-" | "=" | "8" => {
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
                if new_idx < 0 {
                    new_idx += 12;
                } else if new_idx >= 12 {
                    new_idx -= 12;
                }
                self.left_hand_start_note_idx = new_idx as usize;
                
                // 영역이 바뀌면 이전 영역에 눌려있던 키들 해제
                self.release_keys_for_changed_note_idx(ctx, old_idx, self.left_hand_start_note_idx, self.left_hand_octave, true);
                
                // 키 매핑 재생성
                self.recreate_key_mappings();
                true
            },
            PianoMsg::ChangeRightHandStartNote(delta) => {
                let old_idx = self.right_hand_start_note_idx;
                let mut new_idx = self.right_hand_start_note_idx as i32 + delta;
                if new_idx < 0 {
                    new_idx += 12;
                } else if new_idx >= 12 {
                    new_idx -= 12;
                }
                self.right_hand_start_note_idx = new_idx as usize;
                
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
                    
                    // 키 상태 불일치 수정
                    if key.is_pressed != has_pressed_key {
                        key.is_pressed = has_pressed_key;
                        updated = true;
                        
                        // 눌려있지 않아야 하는데 눌려있으면 소리 중지
                        if !has_pressed_key {
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
                true
            },
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            // 첫 렌더링 시 키보드 이벤트 리스너 등록
            let document = web_sys::window().unwrap().document().unwrap();
            
            // 키 다운 이벤트 핸들러
            let link_down = ctx.link().clone();
            let keydown_callback = Closure::wrap(Box::new(move |event: KeyboardEvent| {
                // 기본 동작 방지(페이지 스크롤 등)
                event.prevent_default();
                event.stop_propagation();
                
                let key = event.key();
                console::log_1(&format!("Key down: {}", key).into());
                link_down.send_message(PianoMsg::KeyboardKeyDown(key));
                
                // 강제로 키 상태 업데이트 요청
                let link = link_down.clone();
                let timeout = Timeout::new(10, move || {
                    link.send_message(PianoMsg::ForceKeyUpdate);
                });
                timeout.forget();
            }) as Box<dyn FnMut(KeyboardEvent)>);
            
            // 키 업 이벤트 핸들러
            let link_up = ctx.link().clone();
            let keyup_callback = Closure::wrap(Box::new(move |event: KeyboardEvent| {
                // 기본 동작 방지
                event.prevent_default();
                event.stop_propagation();
                
                let key = event.key();
                console::log_1(&format!("Key up: {}", key).into());
                link_up.send_message(PianoMsg::KeyboardKeyUp(key));
                
                // 강제로 키 상태 업데이트 요청
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
            self._keyboard_listener = Some((keydown_callback, keyup_callback));
            
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
                                                onmousedown={ctx.link().callback(move |_| PianoMsg::KeyPressed(i))}
                                                onmouseup={ctx.link().callback(move |_| PianoMsg::KeyReleased(i))}
                                                onmouseleave={ctx.link().callback(move |_| PianoMsg::KeyReleased(i))}
                                                onmouseout={ctx.link().callback(move |_| PianoMsg::KeyReleased(i))}
                                                ontouchstart={ctx.link().callback(move |_| PianoMsg::KeyPressed(i))}
                                                ontouchend={ctx.link().callback(move |_| PianoMsg::KeyReleased(i))}
                                                ontouchcancel={ctx.link().callback(move |_| PianoMsg::KeyReleased(i))}
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
                                                onmousedown={ctx.link().callback(move |_| PianoMsg::KeyPressed(i))}
                                                onmouseup={ctx.link().callback(move |_| PianoMsg::KeyReleased(i))}
                                                onmouseleave={ctx.link().callback(move |_| PianoMsg::KeyReleased(i))}
                                                onmouseout={ctx.link().callback(move |_| PianoMsg::KeyReleased(i))}
                                                ontouchstart={ctx.link().callback(move |_| PianoMsg::KeyPressed(i))}
                                                ontouchend={ctx.link().callback(move |_| PianoMsg::KeyReleased(i))}
                                                ontouchcancel={ctx.link().callback(move |_| PianoMsg::KeyReleased(i))}
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
        let left_hand_keys = ["z", "x", "c", "v", "a", "s", "d", "f", "q", "w", "e", "r", "t"];
        let right_hand_keys = ["n", "m", ",", ".", "j", "k", "l", ";", "u", "i", "o", "p", "["];
        
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