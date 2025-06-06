use crate::dashboard::{Dashboard, DashboardItem, DashboardLayout};
use crate::routes::{switch, Route};
use gloo::events::EventListener;
use js_sys::{Object};
use log::info;
use std::collections::VecDeque;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AnalyserNode, AudioContext, MediaStream,
    MediaStreamConstraints, CustomEvent, CustomEventInit,
};
use yew::prelude::*;
use yew_router::prelude::*;

// tools 모듈 선언
mod tools {
    pub mod pitch_plot;
    pub mod amplitude_visualizer;
    pub mod metronome;
    pub mod scale_generator;
    pub mod piano;
}

// tools 모듈 컴포넌트 import
use crate::tools::pitch_plot::PitchPlot;
use crate::tools::amplitude_visualizer::AmplitudeVisualizer;
use crate::tools::metronome::Metronome;
use crate::tools::scale_generator::ScaleGenerator;
use crate::tools::piano::Piano;

mod dashboard;
mod routes;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = navigator, js_name = mediaDevices)]
    pub static MEDIA_DEVICES: web_sys::MediaDevices;
}

// 🎶 주어진 주파수를 가장 가까운 음으로 변환하는 함수
fn frequency_to_note(freq: f64) -> &'static str {
    let notes = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let a4 = 440.0;
    let n = ((freq / a4).log2() * 12.0).round();
    let index = (((n as isize) + 69) % 12) as usize;
    notes[index]
}

fn frequency_to_note_octave(freq: f64) -> String {
    let notes = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let a4 = 440.0;
    let n = (12.0 * (freq / a4).log2()).round() as i32;
    let midi_number = n + 69;

    if midi_number < 24 || midi_number > 96 {
        return "Out of range".to_string(); // C1 ~ C6에 해당 (MIDI 24-96)
    }

    let note = notes[(midi_number % 12) as usize];
    let octave = midi_number / 12 - 1;

    format!("{}{}", note, octave)
}

fn analyze_pitch_autocorrelation(
    buffer: &[f32],
    sample_rate: f64,
    sensitivity: f32,
) -> Option<f64> {
    const MIN_FREQ: f64 = 32.0; // C1 주파수에 가까운 값 (32.7Hz)
    const MAX_FREQ: f64 = 1050.0; // C6 주파수에 가까운 값 (1046.5Hz)

    let rms = (buffer.iter().map(|&x| x * x).sum::<f32>() / buffer.len() as f32).sqrt();
    if rms < sensitivity {
        return None;
    }

    let min_lag = (sample_rate / MAX_FREQ) as usize;
    let max_lag = (sample_rate / MIN_FREQ) as usize;

    let mut best_lag = 0;
    let mut max_corr = 0.0;

    for lag in min_lag..=max_lag {
        let mut sum = 0.0;
        for i in 0..(buffer.len() - lag) {
            sum += buffer[i] * buffer[i + lag];
        }

        if sum > max_corr {
            max_corr = sum;
            best_lag = lag;
        }
    }

    if best_lag == 0 {
        return None;
    }

    let frequency = sample_rate / best_lag as f64;

    if frequency < MIN_FREQ || frequency > MAX_FREQ {
        return None;
    }

    Some(frequency)
}

// multi-frequency 분석 함수 추가
fn analyze_multiple_frequencies(
    buffer: &[f32],
    sample_rate: f64,
    sensitivity: f32,
) -> Vec<(f64, f32)> {
    // RMS_THRESHOLD 대신 전달된 sensitivity 사용
    // const RMS_THRESHOLD: f32 = 0.01;
    const MIN_FREQ: f64 = 32.0; // C1 주파수에 가까운 값 (32.7Hz)
    const MAX_FREQ: f64 = 1050.0; // C6 주파수에 가까운 값 (1046.5Hz)
    const PEAK_THRESHOLD: f32 = 0.7; // 최대 상관관계 대비 임계값
    const ABSOLUTE_MIN_FREQ: f64 = 30.0; // 검출 가능한 절대 최소 주파수 (C1보다 약간 낮게)
    const ABSOLUTE_MAX_FREQ: f64 = 1100.0; // 검출 가능한 절대 최대 주파수 (C6보다 약간 높게)

    let rms = (buffer.iter().map(|&x| x * x).sum::<f32>() / buffer.len() as f32).sqrt();
    if rms < sensitivity {
        return Vec::new();
    }

    // 검출 가능한 절대 범위로 lag 범위 계산
    let absolute_min_lag = (sample_rate / ABSOLUTE_MAX_FREQ).max(1.0) as usize;
    let absolute_max_lag = (sample_rate / ABSOLUTE_MIN_FREQ) as usize;

    // 버퍼 길이보다 큰 lag는 계산할 수 없으므로 제한
    let absolute_max_lag = absolute_max_lag.min(buffer.len() - 1);

    // min_lag가 max_lag보다 크면 값을 교체하여 오류 방지
    let (absolute_min_lag, absolute_max_lag) = if absolute_min_lag > absolute_max_lag {
        (1, absolute_min_lag.min(buffer.len() - 1))
    } else {
        (absolute_min_lag, absolute_max_lag)
    };

    // 상관관계 계산 범위는 넓게 잡되, 유효 주파수 판정은 MIN_FREQ~MAX_FREQ로 제한
    let target_min_lag = (sample_rate / MAX_FREQ) as usize;
    let target_max_lag = (sample_rate / MIN_FREQ) as usize;

    // 모든 lag에 대한 상관관계 계산 (넓은 범위)
    let mut correlations = Vec::with_capacity(absolute_max_lag + 1);
    correlations.push(0.0); // 0 lag 값

    for lag in 1..=absolute_max_lag {
        let mut sum = 0.0;
        for i in 0..(buffer.len() - lag) {
            sum += buffer[i] * buffer[i + lag];
        }
        correlations.push(sum);
    }

    // 모든 lag에 대한 상관관계 값 중 최댓값 찾기
    let max_corr = if absolute_min_lag < absolute_max_lag {
        *correlations
            .iter()
            .skip(absolute_min_lag)
            .take(absolute_max_lag - absolute_min_lag)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(&0.0)
    } else {
        // min_lag가 max_lag보다 크거나 같은 경우
        0.0
    };

    // 임계값 설정
    let threshold = max_corr * PEAK_THRESHOLD;

    // 피크 찾기 (전체 범위에서)
    let mut peaks = Vec::new();
    for lag in absolute_min_lag..=absolute_max_lag {
        let corr = correlations[lag];

        // 주변 값보다 큰지 확인 (피크 찾기)
        if corr > threshold
            && (lag <= absolute_min_lag + 1 || corr > correlations[lag - 1])
            && (lag >= absolute_max_lag - 1 || corr > correlations[lag + 1])
        {
            let frequency = sample_rate / lag as f64;

            // 주파수가 범위를 벗어나면 명확히 표시
            let amplitude = (corr / max_corr) as f32;

            if frequency >= MIN_FREQ && frequency <= MAX_FREQ {
                // 정상 범위 주파수는 그대로 추가
                peaks.push((frequency, amplitude));
            } else {
                // 범위 밖 주파수는 특별히 표시 (진폭에 0.5 곱하기)
                // 이는 UI에서 범위 밖 주파수를 표시하되 약하게 표시하는데 사용할 수 있음
                peaks.push((frequency, amplitude * 0.5));
            }
        }
    }

    // 진폭 기준 내림차순 정렬
    peaks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    peaks
}

// 🎤 실시간 피치 분석기
pub struct PitchAnalyzer {
    audio_ctx: Option<AudioContext>,
    analyser: Option<AnalyserNode>,
    _stream: Option<MediaStream>,
    pitch: String,
    prev_freqs: VecDeque<f64>,
    // 여러 주파수를 저장하는 이력 - (timestamp, [(frequency, amplitude)])
    history: VecDeque<(f64, Vec<(f64, f32)>)>,
    canvas_ref: NodeRef,
    elapsed_time: f64,
    current_freq: f64,                        // 🔥 가장 강한 주파수
    sensitivity: f32,                         // 🎚️ 마이크 입력 감도 설정
    show_links: bool,                         // 🔗 링크 표시 여부
    mic_active: bool,                         // 🎤 마이크 활성화 상태
    monitor_active: bool,                     // 🔊 마이크 모니터링 활성화 상태
    speaker_node: Option<web_sys::GainNode>,  // 스피커 출력용 노드
    
    // 오디오 녹음 관련 필드
    is_recording: bool,                       // 녹음 중인지 여부
    is_playing: bool,                         // 재생 중인지 여부
    recorder: Option<web_sys::MediaRecorder>, // 미디어 레코더
    recorded_chunks: Vec<web_sys::Blob>,      // 녹음된 오디오 청크
    recorded_audio_url: Option<String>,       // 녹음된 오디오 URL
    audio_element: Option<web_sys::HtmlAudioElement>, // 오디오 재생 요소
    playback_time: f64,                       // 재생 위치 (초)
    last_recording_time: f64,                 // 마지막 녹음 위치 (초)
    
    // 인터벌 타이머 핸들 추가
    playback_interval: Option<gloo::timers::callback::Interval>,
    recording_start_time: f64,   // 녹음 시작 시간 (audio_ctx 기준)
    
    // 분석 인터벌 추가
    analysis_interval: Option<gloo::timers::callback::Interval>,
    
    // 화면 고정 상태 추가
    is_frozen: bool,
    
    // 최대 녹음 시간 타이머 추가
    max_recording_timer: Option<gloo::timers::callback::Timeout>,
    
    // 녹음 생성 시간 (파일명 생성용)
    created_at_time: f64,
    
    // 진폭 시각화 관련 필드 추가
    amplitude_data: Option<Vec<f32>>,         // 현재 진폭 데이터 배열
    // 진폭 히스토리를 (시간, 진폭 데이터 배열) 형태로 저장
    amplitude_history: VecDeque<(f64, Vec<f32>)>,  // 진폭 히스토리 (시간, 진폭 데이터)
    current_rms: f32,                         // 현재 RMS 레벨
}

// PitchAnalyzer 일반 메서드 구현
impl PitchAnalyzer {
    // 최대 녹음 시간 상수 (10분 = 600초)
    const MAX_RECORDING_TIME: u32 = 600;
    
    // 재생 시간 UI 업데이트 메서드
    fn update_playback_time_ui(&self, time: f64) {
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                // 재생 시간 업데이트 이벤트 발행
                let mut detail = Object::new();
                // currentTime 속성 설정
                let _ = js_sys::Reflect::set(
                    &detail,
                    &JsValue::from_str("currentTime"),
                    &JsValue::from_f64(time),
                );
                // duration 속성 설정
                let _ = js_sys::Reflect::set(
                    &detail,
                    &JsValue::from_str("duration"),
                    &JsValue::from_f64(self.last_recording_time),
                );
                // 녹음 중인지 여부 설정
                let _ = js_sys::Reflect::set(
                    &detail, 
                    &JsValue::from_str("isRecording"),
                    &JsValue::from_bool(self.is_recording),
                );
                
                let event = CustomEvent::new_with_event_init_dict(
                    "playbackTimeUpdate",
                    CustomEventInit::new()
                        .bubbles(true)
                        .detail(&detail),
                ).unwrap();
                
                let _ = document.dispatch_event(&event);
            }
        }
    }
    
    // 녹음된 오디오가 있는지 확인하는 헬퍼 메서드
    fn has_recorded_audio(&self) -> bool {
        self.recorded_audio_url.is_some() && self.audio_element.is_some()
    }
}

pub enum Msg {
    StartAudio,
    StopAudio,   // 🔇 마이크 비활성화 메시지 추가
    ToggleAudio, // 🎤 마이크 활성화/비활성화 토글
    UpdatePitch,
    AudioReady(AudioContext, AnalyserNode, MediaStream),
    UpdateSensitivity(f32),
    ToggleLinks,   // 🔗 링크 표시 여부 토글
    ToggleMonitor, // 🔊 마이크 모니터링 토글
    UpdateSpeakerVolume(f32), // 🔊 스피커 볼륨 업데이트
    
    // 녹음 관련 메시지
    StartRecording,          // 녹음 시작
    StopRecording,           // 녹음 중지
    RecordingDataAvailable(web_sys::Blob), // 녹음 데이터 가용
    RecordingComplete(String), // 녹음 완료 (오디오 URL)
    DownloadRecording,       // 녹음 파일 다운로드
    
    // 재생 관련 메시지
    TogglePlayback,          // 재생/일시정지 토글
    StartPlayback,           // 재생 시작
    PausePlayback,           // 재생 일시정지
    UpdatePlaybackTime(f64), // 재생 시간 업데이트
    PlaybackEnded,           // 재생 완료
    RecorderReady(web_sys::MediaRecorder), // 새로 추가된 메시지 타입
    
    // 새로운 메시지 타입 추가: 시크 (재생 위치 변경)
    SeekPlayback(f64),
    
    // 녹음 길이 업데이트 메시지 추가
    UpdateRecordingDuration(f64),
    
    // 오디오 위치 초기화 메시지
    ResetAudioPosition,

    // 새 메시지 추가: 오디오 리소스 정리
    StopAudioResources,
    
    // 새 메시지 추가: 컴포넌트 상태 완전 초기화
    ResetComponent,
}

// 컴포넌트 Properties 정의 추가
#[derive(Properties, PartialEq)]
pub struct PitchAnalyzerProps {
    #[prop_or(Some(true))]
    pub show_links: Option<bool>,
}

impl Component for PitchAnalyzer {
    type Message = Msg;
    type Properties = PitchAnalyzerProps;

    fn create(ctx: &Context<Self>) -> Self {
        // 이벤트 리스너 추가 - 커스텀 이벤트 수신
        let link = ctx.link().clone();
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();

        // 마이크 토글 이벤트 리스너
        let toggle_audio_callback = Callback::from(move |_: web_sys::Event| {
            link.send_message(Msg::ToggleAudio);
        });

        let toggle_audio_listener = EventListener::new(&document, "toggleAudio", move |e| {
            toggle_audio_callback.emit(e.clone());
        });

        // 감도 조절 이벤트 리스너
        let sensitivity_link = ctx.link().clone();
        let sensitivity_callback = Callback::from(move |e: web_sys::Event| {
            let custom_event = e.dyn_into::<web_sys::CustomEvent>().unwrap();
            let detail = custom_event.detail();
            let value = js_sys::Number::from(detail).value_of() as f32;
            sensitivity_link.send_message(Msg::UpdateSensitivity(value));
        });

        let sensitivity_listener = EventListener::new(&document, "updateSensitivity", move |e| {
            sensitivity_callback.emit(e.clone());
        });

        // 링크 토글 이벤트 리스너
        let toggle_link = ctx.link().clone();
        let toggle_callback = Callback::from(move |_: web_sys::Event| {
            toggle_link.send_message(Msg::ToggleLinks);
        });

        let toggle_listener = EventListener::new(&document, "toggleLinks", move |e| {
            toggle_callback.emit(e.clone());
        });

        // 모니터링 토글 이벤트 리스너
        let monitor_link = ctx.link().clone();
        let monitor_callback = Callback::from(move |_: web_sys::Event| {
            monitor_link.send_message(Msg::ToggleMonitor);
        });

        let monitor_listener = EventListener::new(&document, "toggleMonitor", move |e| {
            monitor_callback.emit(e.clone());
        });

        // 스피커 볼륨 조절 이벤트 리스너
        let volume_link = ctx.link().clone();
        let volume_callback = Callback::from(move |e: web_sys::Event| {
            let custom_event = e.dyn_into::<web_sys::CustomEvent>().unwrap();
            let detail = custom_event.detail();
            let value = js_sys::Number::from(detail).value_of() as f32;
            volume_link.send_message(Msg::UpdateSpeakerVolume(value));
        });

        let volume_listener = EventListener::new(&document, "updateSpeakerVolume", move |e| {
            volume_callback.emit(e.clone());
        });

        // 재생 토글 이벤트 리스너
        let playback_link = ctx.link().clone();
        let playback_callback = Callback::from(move |e: web_sys::Event| {
            let custom_event = e.dyn_into::<web_sys::CustomEvent>().unwrap();
            let detail = custom_event.detail();
            let is_playing = js_sys::Boolean::from(detail).value_of();
            
            if is_playing {
                playback_link.send_message(Msg::StartPlayback);
            } else {
                playback_link.send_message(Msg::PausePlayback);
            }
        });
        
        let playback_listener = EventListener::new(&document, "togglePlayback", move |e| {
            playback_callback.emit(e.clone());
        });
        
        // 재생 시크 이벤트 리스너 추가
        let seek_link = ctx.link().clone();
        let seek_callback = Callback::from(move |e: web_sys::Event| {
            let custom_event = e.dyn_into::<web_sys::CustomEvent>().unwrap();
            let detail = custom_event.detail();
            let progress = js_sys::Number::from(detail).value_of() as f64;
            
            // 진행률 값 검증 (0.0 ~ 1.0 범위로 제한)
            let progress = progress.max(0.0).min(1.0);
            
            // SeekPlayback 메시지 전송
            seek_link.send_message(Msg::SeekPlayback(progress));
        });
        
        let seek_listener = EventListener::new(&document, "seekPlayback", move |e| {
            seek_callback.emit(e.clone());
        });
        
        // 다운로드 이벤트 리스너 추가
        let download_link = ctx.link().clone();
        let download_callback = Callback::from(move |_: web_sys::Event| {
            download_link.send_message(Msg::DownloadRecording);
        });
        
        let download_listener = EventListener::new(&document, "downloadRecording", move |e| {
            download_callback.emit(e.clone());
        });
        
        // 오디오 리소스 정리 이벤트 리스너 추가
        let resources_link = ctx.link().clone();
        let resources_callback = Callback::from(move |_: web_sys::Event| {
            resources_link.send_message(Msg::StopAudioResources);
        });
        
        let resources_listener = EventListener::new(&document, "stopAudioResources", move |e| {
            resources_callback.emit(e.clone());
        });
        
        // 컴포넌트 상태 초기화 이벤트 리스너 추가
        let reset_link = ctx.link().clone();
        let reset_callback = Callback::from(move |_: web_sys::Event| {
            reset_link.send_message(Msg::ResetComponent);
        });
        
        let reset_listener = EventListener::new(&document, "resetPitchAnalyzer", move |e| {
            reset_callback.emit(e.clone());
        });
        
        // 모든 이벤트 리스너 forget 호출
        download_listener.forget();
        seek_listener.forget();
        playback_listener.forget();
        toggle_audio_listener.forget();
        sensitivity_listener.forget();
        toggle_listener.forget();
        monitor_listener.forget();
        volume_listener.forget();
        resources_listener.forget();
        reset_listener.forget();

        // Props에서 show_links 값 가져오기
        let show_links = ctx.props().show_links.unwrap_or(true);

        Self {
            audio_ctx: None,
            analyser: None,
            _stream: None,
            pitch: "🎤 음성 입력 대기...".to_string(),
            prev_freqs: VecDeque::with_capacity(5),
            history: VecDeque::new(),
            canvas_ref: NodeRef::default(),
            elapsed_time: 0.0,
            current_freq: 0.0,
            sensitivity: 0.01,     // 기본 감도 값
            show_links,            // props에서 가져온 값으로 초기화
            mic_active: false,     // 처음에는 마이크 비활성화 상태
            monitor_active: false, // 처음에는 모니터링 비활성화 상태
            speaker_node: None,    // 스피커 노드는 초기화되지 않음
            
            // 오디오 녹음 관련 필드
            is_recording: false,                       // 녹음 중인지 여부
            is_playing: false,                         // 재생 중인지 여부
            recorder: None::<web_sys::MediaRecorder>,  // 미디어 레코더
            recorded_chunks: Vec::new(),                // 녹음된 오디오 청크
            recorded_audio_url: None,                   // 녹음된 오디오 URL
            audio_element: None,                         // 오디오 재생 요소
            playback_time: 0.0,                           // 재생 위치 (초)
            last_recording_time: 0.0,                     // 마지막 녹음 위치 (초)
            
            // 인터벌 타이머 핸들 추가
            playback_interval: None,
            recording_start_time: 0.0,   // 녹음 시작 시간 (audio_ctx 기준)
            
            // 분석 인터벌 추가
            analysis_interval: None,
            
            // 화면 고정 상태 추가
            is_frozen: false,
            
            // 최대 녹음 시간 타이머 추가
            max_recording_timer: None,
            
            // 녹음 생성 시간 초기화 (현재 시간으로)
            created_at_time: js_sys::Date::new_0().get_time(),
            
            // 진폭 시각화 관련 필드 추가
            amplitude_data: None,
            amplitude_history: VecDeque::with_capacity(1000),
            current_rms: 0.0,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::StartAudio => {
                // 이미 활성화된 경우 무시
                if self.mic_active {
                    return false;
                }

                // 기존 녹음 데이터 초기화
                self.recorded_chunks.clear();
                
                let link = ctx.link().clone();
                let mut constraints = MediaStreamConstraints::new();
                constraints.set_audio(&JsValue::TRUE);

                let user_media_promise = MEDIA_DEVICES
                    .get_user_media_with_constraints(&constraints)
                    .expect("Failed to request user media");

                wasm_bindgen_futures::spawn_local(async move {
                    match JsFuture::from(user_media_promise).await {
                        Ok(stream_value) => {
                            info!("got user media stream");
                            let stream = MediaStream::from(stream_value);
                            let audio_ctx =
                                AudioContext::new().expect("Failed to create AudioContext");
                            let analyser = audio_ctx
                                .create_analyser()
                                .expect("Failed to create AnalyserNode");
                            let source = audio_ctx
                                .create_media_stream_source(&stream)
                                .expect("Failed to create MediaStreamAudioSourceNode");

                            analyser.set_fft_size(2048);
                            source
                                .connect_with_audio_node(&analyser)
                                .expect("Failed to connect audio source");

                            // 분석기, 스트림, 컨텍스트를 Msg에 담아 보냄
                            link.send_message(Msg::AudioReady(audio_ctx, analyser, stream.clone()));
                            
                            // 마이크 활성화와 함께 녹음 시작
                            link.send_message(Msg::StartRecording);
                            
                            // MediaRecorder 설정
                            let recorder_options = web_sys::MediaRecorderOptions::new();
                            // 오디오 품질을 높이기 위해 bitsPerSecond 값 설정 (높은 비트레이트)
                            let mut options_obj = js_sys::Object::new();
                            js_sys::Reflect::set(&options_obj, &JsValue::from_str("audioBitsPerSecond"), &JsValue::from_f64(128000.0))
                                .expect("Failed to set audioBitsPerSecond");
                            js_sys::Reflect::set(&options_obj, &JsValue::from_str("mimeType"), &JsValue::from_str("audio/webm;codecs=opus"))
                                .expect("Failed to set mimeType");

                            // options_obj를 recorder_options로 변환
                            let recorder_options = options_obj.unchecked_into::<web_sys::MediaRecorderOptions>();

                            if let Ok(recorder) = web_sys::MediaRecorder::new_with_media_stream_and_media_recorder_options(&stream, &recorder_options) {
                                // 데이터 가용 이벤트 핸들러 설정
                                let link_clone = link.clone();
                                let ondataavailable = Closure::wrap(Box::new(move |event: web_sys::Event| {
                                    let blob_event = event.dyn_into::<web_sys::BlobEvent>().unwrap();
                                    if let Some(blob) = blob_event.data() {
                                        link_clone.send_message(Msg::RecordingDataAvailable(blob));
                                    }
                                }) as Box<dyn FnMut(web_sys::Event)>);
                                
                                // 녹음 완료 이벤트 핸들러 설정
                                let link_clone = link.clone();
                                let onstop = Closure::wrap(Box::new(move |_: web_sys::Event| {
                                    // 녹음이 중지되면 명시적으로 중지됐다는 로그 기록
                                    web_sys::console::log_1(&"레코더 중지 이벤트 발생 - 사후 처리 시작".into());
                                }) as Box<dyn FnMut(web_sys::Event)>);
                                
                                recorder.set_ondataavailable(Some(ondataavailable.as_ref().unchecked_ref()));
                                recorder.set_onstop(Some(onstop.as_ref().unchecked_ref()));
                                
                                // 이벤트 핸들러 메모리 릭 방지를 위해 forget 호출
                                ondataavailable.forget();
                                onstop.forget();
                                
                                // 50ms 간격으로 데이터 수집하도록 설정 (더 작은 청크로 세밀하게 수집)
                                // 이전보다 더 짧은 간격으로 설정하여 데이터 손실 최소화
                                if let Err(err) = recorder.start_with_time_slice(50) {
                                    web_sys::console::error_1(&format!("Failed to start recorder: {:?}", err).into());
                                } else {
                                    web_sys::console::log_1(&"🎙️ 미디어 레코더 시작 - 50ms 간격으로 데이터 수집".into());
                                }
                                
                                // 레코더 객체를 컴포넌트에 저장
                                link.send_message(Msg::RecorderReady(recorder));
                            } else {
                                web_sys::console::error_1(&"Failed to create MediaRecorder".into());
                            }
                        }
                        Err(err) => {
                            web_sys::console::log_1(&format!("Media error: {:?}", err).into());
                        }
                    }
                });

                false
            }

            Msg::UpdatePitch => {
                if let Some(analyser) = &self.analyser {
                    let mut buffer = vec![0.0f32; analyser.fft_size() as usize];
                    analyser.get_float_time_domain_data(&mut buffer[..]);
                    let sample_rate = 44100.0;
                    
                    // 녹음 시작부터 경과된 시간을 계산 (더 안정적인 방식)
                    let current_time = if let Some(audio_ctx) = &self.audio_ctx {
                        // 녹음 시작 시간 기준으로 경과 시간 계산
                        let ctx_current_time = audio_ctx.current_time();
                        let elapsed = ctx_current_time - self.recording_start_time;
                        
                        // 음수나 너무 큰 값이 나오지 않도록 방어
                        if elapsed >= 0.0 && elapsed < 3600.0 {
                            elapsed
                        } else {
                            // 오류 상황: 기존 시간 + 일정 증분 사용
                            self.elapsed_time + 0.1
                        }
                    } else {
                        // 오디오 컨텍스트가 없으면 기본값 0.1씩 증가
                        self.elapsed_time + 0.1
                    };
                    
                    // 여러 주파수 분석
                    let freqs = analyze_multiple_frequencies(&buffer, sample_rate, self.sensitivity);

                    if !freqs.is_empty() {
                        // 가장 강한 주파수 (첫 번째 요소)
                        let strongest_freq = freqs[0].0;

                        // 평균 계산을 위해 이전 목록에 추가
                        if self.prev_freqs.len() >= 5 {
                            self.prev_freqs.pop_front();
                        }
                        self.prev_freqs.push_back(strongest_freq);
                        let average_freq = self.prev_freqs.iter().sum::<f64>() / self.prev_freqs.len() as f64;
                        self.current_freq = average_freq;

                        let note = frequency_to_note_octave(average_freq);
                        self.pitch = format!("🎶 현재 음: {} ({:.2} Hz)", note, average_freq);

                        // 녹음 중인 경우에만 주파수 기록 업데이트
                        if self.is_recording {
                            // 현재 상대 시간과 함께 주파수 목록 기록
                            self.history.push_back((current_time, freqs));
                            
                            // 로그 출력 (디버깅용)
                            web_sys::console::log_1(&format!("🕒 녹음 경과 시간: {:.2}s, 주파수: {:.2}Hz", current_time, average_freq).into());
                        }
                    } else {
                        self.pitch = "🔇 너무 작은 소리 (무시됨)".to_string();
                        self.prev_freqs.clear();
                        self.current_freq = 0.0;

                        // 녹음 중인 경우에만 빈 주파수 목록 기록
                        if self.is_recording {
                            // 빈 주파수 목록 기록 (시간은 계속 유지)
                            self.history.push_back((current_time, Vec::new()));
                        }
                    }
                    
                    // 외부 참조용 시간 업데이트
                    self.elapsed_time = current_time;
                    
                    // 녹음 중일 때는 UI 업데이트 (게이지 바의 시간 표시 업데이트)
                    if self.is_recording {
                        self.last_recording_time = current_time;
                        self.update_playback_time_ui(current_time);
                    }

                    // 진폭 데이터 처리 추가
                    // RMS(Root Mean Square) 계산 - 진폭의 평균 제곱근
                    let rms = (buffer.iter().map(|&x| x * x).sum::<f32>() / buffer.len() as f32).sqrt();
                    self.current_rms = rms;
                    
                    // 진폭 데이터 저장
                    self.amplitude_data = Some(buffer.clone());
                    
                    // 녹음 중인 경우에만 진폭 히스토리 업데이트
                    if self.is_recording {
                        // 현재 상대 시간과 함께 진폭 데이터 기록 (전체 진폭 데이터 저장)
                        self.amplitude_history.push_back((current_time, buffer.clone()));
                        
                        // 히스토리 크기 제한 (최대 1000개 데이터 포인트 유지)
                        if self.amplitude_history.len() > 1000 {
                            self.amplitude_history.pop_front();
                        }
                    }

                    true
                } else {
                    false
                }
            }

            Msg::AudioReady(audio_ctx, analyser, stream) => {
                self.audio_ctx = Some(audio_ctx);
                self.analyser = Some(analyser);
                self._stream = Some(stream.clone());
                self.mic_active = true;
                self.is_recording = true;

                // 녹음기 초기화
                if let Ok(recorder) = web_sys::MediaRecorder::new_with_media_stream(&stream) {
                    self.recorder = Some(recorder);
                } else {
                    web_sys::console::error_1(&"Failed to create MediaRecorder in AudioReady handler".into());
                }

                // 스트림 복제: 하나는 분석용, 하나는 모니터링용으로 분리
                if let Some(ctx) = &self.audio_ctx {
                    if let Some(stream) = &self._stream {
                        // 웹 오디오 그래프 구성:
                        // 1. 마이크 입력 -> 분석기 (분석 데이터 생성)
                        // 2. 스피커 출력은 필요시 별도로 연결 (ToggleMonitor에서 처리)
                        //
                        // 이렇게 하면 마이크와 스피커가 서로 다른 경로로 처리되어
                        // 에코 캔슬링으로 인한 문제가 발생하지 않습니다.
                        web_sys::console::log_1(&"Audio graph configured for analysis".into());
                    }
                }

                let link = ctx.link().clone();
                
                // 오디오 분석 인터벌 설정 - 녹음 시간 업데이트는 별도로 처리
                let interval = gloo::timers::callback::Interval::new(100, move || {
                    link.send_message(Msg::UpdatePitch);
                });
                
                // 인터벌 핸들 저장
                self.analysis_interval = Some(interval);

                true
            }

            Msg::ToggleLinks => {
                self.show_links = !self.show_links;
                true
            }

            Msg::UpdateSensitivity(value) => {
                self.sensitivity = value;
                true
            }

            Msg::StopAudio => {
                // 녹음 중지 먼저 처리
                if self.is_recording {
                    // 진행 중인 녹음이 있으면 중지 요청만 하고 종료
                    // 실제 정리는 StopRecording 및 RecordingComplete에서 처리됨
                    ctx.link().send_message(Msg::StopRecording);
                    
                    // 녹음 종료가 완료될 때까지 오디오 컨텍스트 정리를 지연시키기 위해
                    // 비동기 처리를 설정
                    let link = ctx.link().clone();
                    
                    // 1초 후 오디오 컨텍스트 정리를 시도 (녹음 종료 처리에 충분한 시간)
                    gloo::timers::callback::Timeout::new(1000, move || {
                        link.send_message(Msg::StopAudioResources);
                    }).forget();
                    
                    // UI 상태 업데이트를 위한 이벤트 발생
                    if let Some(window) = web_sys::window() {
                        if let Some(document) = window.document() {
                            // 마이크 비활성화 이벤트 발생
                            let event = CustomEvent::new_with_event_init_dict(
                                "toggleAudio",
                                CustomEventInit::new()
                                    .bubbles(true)
                                    .detail(&JsValue::from_bool(false)),
                            ).unwrap_or_else(|_| web_sys::CustomEvent::new("toggleAudio").unwrap());
                            
                            let _ = document.dispatch_event(&event);
                            
                            // 컨트롤 버튼 비활성화 이벤트 발생 (명시적으로 분리하여 처리)
                            let disable_event = web_sys::Event::new("disableControlButtons").expect("disableControlButtons 이벤트 생성 실패");
                            if let Err(err) = document.dispatch_event(&disable_event) {
                                web_sys::console::error_1(&format!("disableControlButtons 이벤트 발생 실패: {:?}", err).into());
                            } else {
                                web_sys::console::log_1(&"컨트롤 버튼 비활성화 이벤트 발생 성공 (StopAudio)".into());
                            }
                            
                            web_sys::console::log_1(&"마이크 비활성화 및 컨트롤 버튼 비활성화 이벤트 발생 (StopAudio)".into());
                        }
                    }
                    
                    return true;
                }

                // 최대 녹음 시간 타이머 취소
                self.max_recording_timer = None;

                // 이미 녹음 중이 아니면 즉시 리소스 정리
                // ctx.link().send_message(Msg::StopAudioResources);
                true
            },

            Msg::ToggleAudio => {
                if self.mic_active {
                    // 마이크가  😅활성화된 상태면 비활성화
                    ctx.link().send_message(Msg::StopAudio);
                } else {
                    // 마이크가 비활성화된 상태면 활성화
                    ctx.link().send_message(Msg::StartAudio);
                }

                false
            }

            Msg::ToggleMonitor => {
                // 마이크가 비활성화 상태라면 모니터링을 할 수 없음
                if !self.mic_active {
                    web_sys::console::log_1(
                        &"Cannot toggle monitor without active microphone".into(),
                    );
                    return false;
                }

                self.monitor_active = !self.monitor_active;

                if let (Some(audio_ctx), Some(analyser)) = (&self.audio_ctx, &self.analyser) {
                    if self.monitor_active {
                        // 모니터링 활성화: 새로운 연결 설정
                        if let Some(stream) = &self._stream {
                            // 분석기 노드를 그대로 두고, 스트림에서 새로운 소스 노드를 생성
                            match audio_ctx.clone().create_media_stream_source(stream) {
                                Ok(monitor_source) => {
                                    // 1. 로우패스 필터 생성 (고주파 제거)
                                    match audio_ctx.clone().create_biquad_filter() {
                                        Ok(filter_node) => {
                                            // 로우패스 필터 타입 설정 (0은 lowpass)
                                            filter_node.set_type(web_sys::BiquadFilterType::Lowpass);
                                            filter_node.frequency().set_value(1500.0); // 1.5kHz 이상 감쇠
                                            filter_node.q().set_value(1.0);
                                            
                                            // 2. 딜레이 노드 생성 (약간의 지연 추가)
                                            match audio_ctx.clone().create_delay() {
                                                Ok(delay_node) => {
                                                    // 50ms 딜레이 설정
                                                    delay_node.delay_time().set_value(0.05);
                                                    
                                                    // 3. 게인 노드 생성 (볼륨 조절)
                                                    match audio_ctx.clone().create_gain() {
                                                        Ok(gain_node) => {
                                                            // 볼륨 설정 (피드백 방지를 위해 매우 낮게 설정)
                                                            let gain_param = gain_node.gain();
                                                            gain_param.set_value(0.02); // 2% 볼륨으로 감소
                                                            
                                                            // 오디오 그래프 연결:
                                                            // 소스 -> 필터 -> 딜레이 -> 게인 -> 출력
                                                            
                                                            // 소스를 필터에 연결
                                                            if monitor_source.connect_with_audio_node(&filter_node).is_err() {
                                                                web_sys::console::log_1(&"Failed to connect source to filter".into());
                                                                self.monitor_active = false;
                                                                return false;
                                                            }
                                                            
                                                            // 필터를 딜레이에 연결
                                                            if filter_node.connect_with_audio_node(&delay_node).is_err() {
                                                                web_sys::console::log_1(&"Failed to connect filter to delay".into());
                                                                self.monitor_active = false;
                                                                return false;
                                                            }
                                                            
                                                            // 딜레이를 게인에 연결
                                                            if delay_node.connect_with_audio_node(&gain_node).is_err() {
                                                                web_sys::console::log_1(&"Failed to connect delay to gain".into());
                                                                self.monitor_active = false;
                                                                return false;
                                                            }
                                                            
                                                            // 게인 노드를 출력에 연결
                                                            if gain_node.connect_with_audio_node(&audio_ctx.clone().destination()).is_err() {
                                                                web_sys::console::log_1(&"Failed to connect gain to destination".into());
                                                                self.monitor_active = false;
                                                                return false;
                                                            }
                                                            
                                                            // 스피커 노드 저장 (나중에 연결 해제용)
                                                            self.speaker_node = Some(gain_node);
                                                            web_sys::console::log_1(&"Monitor activated with anti-feedback measures".into());
                                                        }
                                                        Err(_) => {
                                                            web_sys::console::log_1(&"Failed to create gain node".into());
                                                            self.monitor_active = false;
                                                            return false;
                                                        }
                                                    }
                                                }
                                                Err(_) => {
                                                    web_sys::console::log_1(&"Failed to create delay node".into());
                                                    self.monitor_active = false;
                                                    return false;
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            web_sys::console::log_1(&"Failed to create filter node".into());
                                            self.monitor_active = false;
                                            return false;
                                        }
                                    }
                                }
                                Err(_) => {
                                    web_sys::console::log_1(&"Failed to create monitor source".into());
                                    self.monitor_active = false;
                                    return false;
                                }
                            }
                        }
                    } else {
                        // 모니터링 비활성화: 연결 해제
                        if let Some(speaker_node) = &self.speaker_node {
                            // 웹오디오 API는 disconnect() 메서드로 모든 연결을 해제
                            speaker_node.disconnect();
                            self.speaker_node = None;
                            web_sys::console::log_1(&"Monitor deactivated".into());
                        }
                    }
                    return true;
                }

                false
            }

            Msg::UpdateSpeakerVolume(value) => {
                if let Some(gain_node) = &self.speaker_node {
                    // 값이 0.0~1.0 범위를 벗어나지 않도록 보장
                    let volume = value.max(0.0).min(1.0);
                    gain_node.gain().set_value(volume);
                    web_sys::console::log_1(&format!("Speaker volume updated to: {:.2}", volume).into());
                } else {
                    web_sys::console::log_1(&"Cannot update volume - speaker not initialized".into());
                }
                true
            }

            Msg::StartRecording => {
                self.is_recording = true;
                self.is_playing = false;
                self.recorder = None;
                self.recorded_chunks.clear(); // 기존 녹음 데이터 초기화
                self.recorded_audio_url = None;
                self.audio_element = None;
                self.playback_time = 0.0;
                self.last_recording_time = 0.0;
                
                // 녹음 시작 시간 갱신
                self.created_at_time = js_sys::Date::new_0().get_time();
                
                // 화면 고정 해제 - 새로운 녹음 시작 시
                self.is_frozen = false;

                // 녹음 시작 시간 저장
                if let Some(audio_ctx) = &self.audio_ctx {
                    self.recording_start_time = audio_ctx.current_time();
                    web_sys::console::log_1(&format!("녹음 시작 절대 시간: {:.2}초", self.recording_start_time).into());
                } else {
                    self.recording_start_time = 0.0;
                }
                
                // 시간 초기화
                self.elapsed_time = 0.0;
                
                // === 차트 관련 상태 초기화 ===
                self.history.clear();
                self.prev_freqs.clear();
                self.current_freq = 0.0;
                
                // 게이지 바 초기화를 위해 UI 업데이트
                self.update_playback_time_ui(0.0);
                
                // PitchPlot의 재생 위치 초기화를 위한 이벤트 발행
                if let Some(window) = web_sys::window() {
                    if let Some(document) = window.document() {
                        // playbackReset 이벤트 발행: pitch plot의 playback 선을 0초로 초기화
                        let event = web_sys::Event::new("playbackReset").unwrap();
                        let _ = document.dispatch_event(&event);
                        web_sys::console::log_1(&"녹음 시작: playbackReset 이벤트 발행".into());
                    }
                }
                
                // 최대 녹음 시간 타이머 설정 (10분 후 자동 중지)
                let link = ctx.link().clone();
                let max_recording_timer = gloo::timers::callback::Timeout::new(
                    Self::MAX_RECORDING_TIME * 1000, // 밀리초 단위 변환
                    move || {
                        web_sys::console::log_1(&format!("최대 녹음 시간 ({}초) 도달, 자동 중지", Self::MAX_RECORDING_TIME).into());
                        // 녹음 중지 및 마이크 비활성화 메시지 전송
                        link.send_message(Msg::StopRecording);
                        link.send_message(Msg::StopAudio);
                        
                        // 마이크 비활성화 UI 상태 업데이트를 위한 이벤트 발생
                        if let Some(window) = web_sys::window() {
                            if let Some(document) = window.document() {
                                // 마이크 비활성화 이벤트 발생
                                let event = CustomEvent::new_with_event_init_dict(
                                    "toggleAudio",
                                    CustomEventInit::new()
                                        .bubbles(true)
                                        .detail(&JsValue::from_bool(false)),
                                ).unwrap_or_else(|_| web_sys::CustomEvent::new("toggleAudio").unwrap());
                                
                                let _ = document.dispatch_event(&event);
                                web_sys::console::log_1(&"마이크 비활성화 이벤트 발생 (최대 녹음 시간 도달)".into());
                            }
                        }
                        
                        // 사용자에게 알림 표시
                        if let Some(window) = web_sys::window() {
                            let _ = window.alert_with_message(&format!("최대 녹음 시간 ({}초)에 도달하여 녹음이 자동으로 중지되었습니다.", Self::MAX_RECORDING_TIME));
                        }
                    }
                );
                
                // 이전 타이머가 있으면 취소하고 새 타이머 설정
                self.max_recording_timer = Some(max_recording_timer);
                
                web_sys::console::log_1(&"녹음 시작: 시간 초기화됨".into());

                true
            }

            Msg::StopRecording => {
                // 이미 녹음 중지 상태면 무시
                if !self.is_recording {
                    return false;
                }
                
                web_sys::console::log_1(&"⏹️ 녹음 중지 버튼 누름 - pitchplot 업데이트 중단 & 데이터 처리 시작".into());
                
                // 녹음 종료 상태로 변경하되 청크 처리는 아직 진행 중
                self.is_recording = false;
                
                // 최대 녹음 시간 타이머 취소
                self.max_recording_timer = None;
                
                // 화면 고정 활성화 - 녹음 중지 시
                self.is_frozen = true;
                
                // pitch 분석 인터벌 중지
                self.analysis_interval = None;
                web_sys::console::log_1(&"피치 분석 인터벌 중지됨".into());
                
                // 히스토리에 마지막 시간 기록 - 이후 업데이트 중단
                let current_recording_time = self.elapsed_time;
                self.last_recording_time = if current_recording_time > 0.0 && current_recording_time < 3600.0 {
                    current_recording_time
                } else if let Some((last_time, _)) = self.history.back() {
                    *last_time
                } else {
                    1.0 // 안전 기본값
                };
                
                // UI 알림용 "녹음 종료됨" 상태 이벤트 발행
                if let Some(window) = web_sys::window() {
                    if let Some(document) = window.document() {
                        let event = CustomEvent::new_with_event_init_dict(
                            "recordingStateChange",
                            CustomEventInit::new()
                                .bubbles(true)
                                .detail(&JsValue::from_bool(false)),
                        ).unwrap_or_else(|_| web_sys::CustomEvent::new("recordingStateChange").unwrap());
                        
                        let _ = document.dispatch_event(&event);
                    }
                }
                
                // MediaRecorder가 있는 경우에만 처리
                if let Some(recorder) = &self.recorder {
                    // 현재 상태가 녹음 중인 경우에만 중지 요청
                    if recorder.state() == web_sys::RecordingState::Recording {
                        // ondataavailable과 onstop 이벤트 핸들러는 아직 유지
                        // (데이터 수집을 위해 필요함)
                        
                        // 게이지바 상태 업데이트 (게이지는 0으로 초기화하되, 전체 시간 표시는 녹음 시간으로)
                        self.playback_time = 0.0;
                        self.update_playback_time_ui(0.0);
                        
                        // 녹음 중지를 비동기로 처리하고 모든 데이터가 수집될 때까지 기다림
                        let link = ctx.link().clone();
                        let recorder_clone = recorder.clone();
                        
                        web_sys::console::log_1(&"녹음 중지 요청 - 모든 데이터 청크가 수집될 때까지 기다립니다...".into());
                        
                        // 비동기 처리를 위한 Promise 생성
                        let promise = js_sys::Promise::new(&mut move |resolve, _reject| {
                            let recorder_js = recorder_clone.clone();
                            
                            // onstop 이벤트 핸들러 설정 - 모든 데이터가 수집됐을 때 호출됨
                            let onstop_closure = Closure::once(move |_event: web_sys::Event| {
                                web_sys::console::log_1(&"레코더 onstop 이벤트: 모든 데이터 수집 완료".into());
                                // Promise 해결
                                let _ = resolve.call0(&JsValue::NULL);
                            });
                            
                            // 이벤트 핸들러 설정
                            recorder_js.set_onstop(Some(onstop_closure.as_ref().unchecked_ref()));
                            
                            // 녹음 중지 요청
                            if let Err(err) = recorder_js.stop() {
                                web_sys::console::error_1(&format!("녹음 중지 오류: {:?}", err).into());
                            }
                            
                            // 메모리 릭 방지
                            onstop_closure.forget();
                        });
                        
                        // Promise 처리를 위한 Future 변환 및 실행
                        wasm_bindgen_futures::spawn_local(async move {
                            match JsFuture::from(promise).await {
                                Ok(_) => {
                                    web_sys::console::log_1(&"모든 녹음 데이터 수집 완료 - 후처리 시작".into());
                                    // 모든 데이터가 수집되었으므로 레코더 리소스 정리 메시지 전송
                                    link.send_message(Msg::RecordingComplete(String::new()));
                                },
                                Err(err) => {
                                    web_sys::console::error_1(&format!("녹음 데이터 수집 중 오류 발생: {:?}", err).into());
                                    // 오류 발생 시에도 RecordingComplete 메시지 전송하여 정리
                                    link.send_message(Msg::RecordingComplete(String::new()));
                                }
                            }
                        });
                    } else {
                        // 이미 중지된 상태라면 바로 RecordingComplete 호출
                        ctx.link().send_message(Msg::RecordingComplete(String::new()));
                    }
                } else {
                    // 레코더가 없는 경우에도 RecordingComplete 호출
                    ctx.link().send_message(Msg::RecordingComplete(String::new()));
                }
                
                true
            },

            Msg::RecordingDataAvailable(blob) => {
                // 블롭 크기가 0보다 크면 처리
                if blob.size() > 0.0 {
                    self.recorded_chunks.push(blob.clone());
                    
                    // 로그 - 데이터 청크 수신
                    let chunk_size = blob.size();
                    let chunks_count = self.recorded_chunks.len();
                    
                    if self.is_recording {
                        // 녹음 중 - 정상적인 데이터 수집
                        web_sys::console::log_1(&format!("🎙️ 오디오 데이터 청크 수신 (녹음 중) - 크기: {:.2} KB, 총 청크: {}", 
                            chunk_size / 1024.0, chunks_count).into());
                    } else {
                        // 녹음 중지 후 - 나머지 데이터 수집 중
                        web_sys::console::log_1(&format!("🎙️ 오디오 데이터 청크 수신 (녹음 종료 후 정리 중) - 크기: {:.2} KB, 총 청크: {}", 
                            chunk_size / 1024.0, chunks_count).into());
                    }
                } else {
                    // 빈 청크는 무시하지만 로그는 남김
                    web_sys::console::log_1(&"빈 오디오 데이터 청크 수신됨 (무시됨)".into());
                }
                true
            },

            Msg::RecordingComplete(url) => {
                // 녹음 완료
                self.is_recording = false;
                
                // 기존 오디오 요소가 있으면 이벤트 리스너 제거 및 리소스 정리
                if let Some(old_audio) = &self.audio_element {
                    old_audio.set_onloadeddata(None);
                    old_audio.set_onloadedmetadata(None);
                    old_audio.set_onended(None);
                    
                    // URL 리소스 정리
                    if let Some(old_url) = &self.recorded_audio_url {
                        let _ = web_sys::Url::revoke_object_url(old_url);
                    }
                }
                
                // url 파라미터가 비어있는 경우, 직접 녹음된 청크로 URL 생성 (StopRecording에서 전달됨)
                let audio_url = if url.is_empty() {
                    // 데이터 이벤트 핸들러 제거
                    if let Some(recorder) = &self.recorder {
                        // 이벤트 핸들러 제거 및 정리
                        recorder.set_ondataavailable(None);
                        recorder.set_onstop(None);
                        
                        web_sys::console::log_1(&"레코더 정리 완료".into());
                    }
                    
                    // 모든 관련 상태 초기화
                    self.recorder = None;
                    
                    // 녹음된 청크를 결합하여 URL 생성
                    if !self.recorded_chunks.is_empty() {
                        let blobs = js_sys::Array::new();
                        for blob in &self.recorded_chunks {
                            blobs.push(blob);
                        }
                        
                        // 녹음된 청크 수 및 크기 기록
                        let total_chunks = self.recorded_chunks.len();
                        let mut total_size = 0.0;
                        for blob in &self.recorded_chunks {
                            total_size += blob.size();
                        }
                        web_sys::console::log_1(&format!("처리 중인 녹음 청크: {}개, 총 크기: {:.2} KB", 
                            total_chunks, total_size / 1024.0).into());
                        
                        // Blob 배열을 하나의 Blob으로 합치기
                        let mut blob_options = web_sys::BlobPropertyBag::new();
                        blob_options.type_("audio/webm");
                        
                        match web_sys::Blob::new_with_blob_sequence_and_options(&blobs, &blob_options) {
                            Ok(combined_blob) => {
                                // Blob 크기 확인
                                let blob_size = combined_blob.size();
                                web_sys::console::log_1(&format!("생성된 Blob 크기: {:.2} KB", blob_size / 1024.0).into());
                                
                                // Blob URL 생성
                                match web_sys::Url::create_object_url_with_blob(&combined_blob) {
                                    Ok(new_url) => new_url,
                                    Err(err) => {
                                        web_sys::console::error_1(&format!("URL 생성 실패: {:?}", err).into());
                                        return false;
                                    }
                                }
                            },
                            Err(err) => {
                                web_sys::console::error_1(&format!("Blob 결합 실패: {:?}", err).into());
                                return false;
                            }
                        }
                    } else {
                        web_sys::console::error_1(&"처리할 녹음 청크가 없습니다".into());
                        return false;
                    }
                } else {
                    // 이미 생성된 URL이 전달된 경우 그대로 사용
                    url
                };
                
                // 새 URL 저장
                self.recorded_audio_url = Some(audio_url.clone());
                
                // 오디오 요소 생성
                if let Some(window) = web_sys::window() {
                    if let Some(document) = window.document() {
                        if let Ok(element) = document.create_element("audio") {
                            let audio_element: web_sys::HtmlAudioElement = element
                                .dyn_into()
                                .expect("Failed to create audio element");
                            
                            audio_element.set_src(&audio_url);
                            audio_element.set_controls(false);
                            
                            // 재생 종료 이벤트 리스너 추가
                            let link = ctx.link().clone();
                            let onended = Closure::wrap(Box::new(move |_: web_sys::Event| {
                                link.send_message(Msg::PlaybackEnded);
                            }) as Box<dyn FnMut(web_sys::Event)>);
                            
                            // 로드 완료 이벤트 리스너 추가 - 실제 오디오 파일 길이 확인
                            let link_load = ctx.link().clone();
                            let last_recording_time = self.last_recording_time;
                            let onloadedmetadata = Closure::wrap(Box::new(move |e: web_sys::Event| {
                                if let Some(target) = e.target() {
                                    if let Ok(audio) = target.dyn_into::<web_sys::HtmlAudioElement>() {
                                        let actual_duration = audio.duration();
                                        
                                        // 로그로 실제 오디오 길이와 기록된 길이 비교
                                        web_sys::console::log_1(&format!("오디오 메타데이터 로드됨: 실제 길이 = {:.2}초, 기록된 길이 = {:.2}초", 
                                            actual_duration, last_recording_time).into());
                                        
                                        // 실제 오디오 길이로 last_recording_time 업데이트
                                        link_load.send_message(Msg::UpdateRecordingDuration(actual_duration));
                                    }
                                }
                            }) as Box<dyn FnMut(web_sys::Event)>);
                            
                            audio_element.set_onended(Some(onended.as_ref().unchecked_ref()));
                            audio_element.set_onloadedmetadata(Some(onloadedmetadata.as_ref().unchecked_ref()));
                            onended.forget();
                            onloadedmetadata.forget();
                            
                            // 오디오 요소에 고유 ID 부여 (추적 및 선택 가능하도록)
                            audio_element.set_id("pitch-analyzer-audio");
                            
                            // 오디오 요소를 DOM에 추가 (숨겨진 컨테이너에)
                            if let Some(document) = web_sys::window().unwrap().document() {
                                // 오디오 컨테이너가 있는지 확인하고 없으면 생성
                                let container_id = "pitch-analyzer-audio-container";
                                if document.get_element_by_id(container_id).is_none() {
                                    if let Ok(container) = document.create_element("div") {
                                        // 컨테이너 설정
                                        container.set_id(container_id);
                                        // 화면에 표시되지 않도록 스타일 설정
                                        if let Ok(_) = container.set_attribute("style", "display: none; position: absolute; width: 0; height: 0;") {
                                            // 문서에 추가
                                            if let Some(body) = document.body() {
                                                let _ = body.append_child(&container);
                                                web_sys::console::log_1(&"오디오 컨테이너 DOM에 추가됨".into());
                                            }
                                        }
                                    }
                                }
                                
                                // 기존 오디오 요소가 있으면 제거
                                if let Some(old_audio) = document.get_element_by_id("pitch-analyzer-audio") {
                                    if let Some(parent) = old_audio.parent_node() {
                                        let _ = parent.remove_child(&old_audio);
                                    }
                                }
                                
                                // 새 오디오 요소를 컨테이너에 추가
                                if let Some(container) = document.get_element_by_id(container_id) {
                                    let _ = container.append_child(&audio_element);
                                    web_sys::console::log_1(&"오디오 요소 DOM에 추가됨".into());
                                }
                            }
                            
                            self.audio_element = Some(audio_element);
                            
                            // 녹음 데이터 초기화 - 메모리 누수 방지
                            self.recorded_chunks.clear();
                        }
                    }
                }
                
                // 녹음 완료 이벤트 발행
                if let Some(window) = web_sys::window() {
                    if let Some(document) = window.document() {
                        let event = CustomEvent::new_with_event_init_dict(
                            "recordingComplete",
                            CustomEventInit::new()
                                .bubbles(true)
                                .detail(&JsValue::from_str(&audio_url)),
                        ).unwrap_or_else(|_| web_sys::CustomEvent::new("recordingComplete").unwrap());
                        
                        let _ = document.dispatch_event(&event);
                        web_sys::console::log_1(&"recordingComplete 이벤트 발행".into());
                    }
                }
                
                true
            },

            Msg::TogglePlayback => {
                if self.is_playing {
                    ctx.link().send_message(Msg::PausePlayback);
                } else {
                    ctx.link().send_message(Msg::StartPlayback);
                }
                false
            }

            Msg::StartPlayback => {
                // 녹음 중이면 재생 불가
                if self.is_recording {
                    web_sys::console::log_1(&"녹음 중에는 재생할 수 없습니다".into());
                    return false;
                }
                
                // 화면 고정 해제 - 재생 중에는 화면이 업데이트되어야 함
                self.is_frozen = false;
                
                // 이미 재생 중인 경우 중복 호출 방지
                if self.is_playing {
                    web_sys::console::log_1(&"이미 재생 중입니다".into());
                    return false;
                }
                
                if let Some(audio_element) = &self.audio_element {
                    web_sys::console::log_1(&format!("StartPlayback: 오디오 요소={:?}, ready_state={}", 
                        audio_element, audio_element.ready_state()).into());
                    
                    // 기존 인터벌이 있으면 제거
                    self.playback_interval = None;
                    
                    // 오디오 데이터가 로드되었는지 확인
                    if audio_element.ready_state() < 2 { // HAVE_CURRENT_DATA = 2
                        web_sys::console::log_1(&"오디오 데이터가 아직 로드되지 않음. loadeddata 리스너 설정".into());
                        
                        // 아직 로드 중이면 로드 완료 후 재생을 시도하도록 이벤트 리스너 추가
                        let link = ctx.link().clone();
                        let audio_element_clone = audio_element.clone();
                        let onloadeddata = Closure::wrap(Box::new(move |_: web_sys::Event| {
                            web_sys::console::log_1(&"오디오 데이터 로드 완료 콜백 실행".into());
                            // 로드 완료 후 재생 시도
                            if let Err(err) = audio_element_clone.play() {
                                web_sys::console::error_1(&format!("로드 후 재생 시작 실패: {:?}", err).into());
                            } else {
                                web_sys::console::log_1(&"로드 후 재생 시작됨".into());
                                // 재생 성공 시 플래그 설정
                                link.send_message(Msg::StartPlayback);
                            }
                        }) as Box<dyn FnMut(web_sys::Event)>);
                        
                        // 기존 리스너 제거 후 새 리스너 설정
                        audio_element.set_onloadeddata(None);
                        audio_element.set_onloadeddata(Some(onloadeddata.as_ref().unchecked_ref()));
                        onloadeddata.forget();
                        
                        // 로딩 중임을 알림
                        web_sys::console::log_1(&"오디오 데이터 로드 대기 중...".into());
                        return true;
                    }
                    
                    // 오디오 요소가 있고 데이터가 로드되었으면 재생 시작
                    web_sys::console::log_1(&"오디오 데이터 로드됨, 재생 시작".into());
                    
                    // 재생이 끝나서 다시 시작하는 경우만 처음부터 재생
                    if audio_element.ended() {
                        audio_element.set_current_time(0.0);
                        self.playback_time = 0.0;
                        web_sys::console::log_1(&"재생이 끝난 상태에서 다시 시작하므로 처음부터 재생".into());
                    } else {
                        // 일시 정지된 위치에서 계속 재생
                        web_sys::console::log_1(&format!("재생 위치 유지: {:.2}초", audio_element.current_time()).into());
                    }
                    
                    // 기존 이벤트 리스너들 명시적으로 제거
                    audio_element.set_onended(None);
                    
                    // 종료 이벤트 새로 설정
                    let link = ctx.link().clone();
                    let onended = Closure::wrap(Box::new(move |_: web_sys::Event| {
                        web_sys::console::log_1(&"재생 종료 이벤트 발생".into());
                        link.send_message(Msg::PlaybackEnded);
                    }) as Box<dyn FnMut(web_sys::Event)>);
                    audio_element.set_onended(Some(onended.as_ref().unchecked_ref()));
                    onended.forget();
                    
                    // 재생 상태 설정 (재생 시작 전에 설정)
                    self.is_playing = true;
                    
                    // 재생 시작
                    if let Err(err) = audio_element.play() {
                        web_sys::console::error_1(&format!("재생 시작 실패: {:?}", err).into());
                        self.is_playing = false;
                        return false;
                    }
                    
                    web_sys::console::log_1(&format!("재생 시작됨, is_playing={}", self.is_playing).into());
                    
                    // 재생 상태 이벤트 발행
                    if let Some(window) = web_sys::window() {
                        if let Some(document) = window.document() {
                            let event = CustomEvent::new_with_event_init_dict(
                                "playbackStateChange",
                                CustomEventInit::new()
                                    .bubbles(true)
                                    .detail(&JsValue::from_bool(true)),
                            ).unwrap();
                            let _ = document.dispatch_event(&event);
                        }
                    }
                    
                    // 재생 시간 UI 업데이트 (초기 로딩 시)
                    self.update_playback_time_ui(audio_element.current_time());
                    
                    // 재생 상태 업데이트를 위한 인터벌 설정
                    let link = ctx.link().clone();
                    let audio_element_clone = audio_element.clone();
                    
                    // 새 인터벌 생성
                    let interval = gloo::timers::callback::Interval::new(30, move || {
                        // 오디오 요소가 아직 유효한지 확인
                        if audio_element_clone.ended() {
                            web_sys::console::log_1(&"재생 종료 감지됨 (인터벌)".into());
                            link.send_message(Msg::PlaybackEnded);
                            return;
                        }
                        
                        // 현재 재생 시간 가져오기
                        let current_time = audio_element_clone.current_time();
                        
                        // 시간 업데이트 메시지 전송 - 모든 시간값 전송
                        link.send_message(Msg::UpdatePlaybackTime(current_time));
                    });
                    
                    // 인터벌 핸들 저장
                    self.playback_interval = Some(interval);
                    
                    true
                } else {
                    // 오디오 요소가 없으면 재생 불가
                    web_sys::console::error_1(&"재생할 오디오 요소가 없음".into());
                    false
                }
            }

            Msg::PausePlayback => {
                // 이미 정지 상태면 중복 호출 무시
                if !self.is_playing {
                    return false;
                }
                
                if let Some(audio_element) = &self.audio_element {
                    // 현재 재생 시간 기록
                    self.playback_time = audio_element.current_time();
                    web_sys::console::log_1(&format!("일시 정지 시점 시간 저장: {:.2}초", self.playback_time).into());
                    
                    // 오디오 요소가 있으면 일시정지
                    if let Err(err) = audio_element.pause() {
                        web_sys::console::error_1(&format!("재생 일시정지 실패: {:?}", err).into());
                        return false;
                    }
                    
                    // 인터벌 타이머 제거
                    self.playback_interval = None;
                    
                    // 상태 업데이트
                    self.is_playing = false;
                    web_sys::console::log_1(&"재생 일시정지됨".into());
                    
                    // 재생 상태 이벤트 발행
                    if let Some(window) = web_sys::window() {
                        if let Some(document) = window.document() {
                            let event = CustomEvent::new_with_event_init_dict(
                                "playbackStateChange",
                                CustomEventInit::new()
                                    .bubbles(true)
                                    .detail(&JsValue::from_bool(false)),
                            ).unwrap();
                            let _ = document.dispatch_event(&event);
                        }
                    }
                    
                    true
                } else {
                    // 오디오 요소가 없으면 일시정지 불가
                    false
                }
            }

            Msg::UpdatePlaybackTime(time) => {
                if !self.is_playing {
                    // 재생 중이 아닌데 호출되면, 이는 잘못된 상태임을 기록하고 무시
                    web_sys::console::log_1(&format!("⚠️ 재생 중이 아닌데 UpdatePlaybackTime 호출됨: {:.2}s", time).into());
                    return false;
                }
                
                // 시간이 너무 작으면 무시 (seek 동작으로 인한 오류 방지)
                if time < 0.001 {
                    web_sys::console::log_1(&"시간이 너무 작아서 무시 (0에 가까움)".into());
                    return false;
                }
                
                // 작은 변화는 무시 (성능 향상)
                if (time - self.playback_time).abs() < 0.05 {
                    return false;
                }
                
                // 재생 시간 업데이트
                self.playback_time = time;
                
                // UI에 재생 시간과 총 녹음 시간 정보 전달
                if let Some(window) = web_sys::window() {
                    if let Some(document) = window.document() {
                        // 재생 시간 업데이트 이벤트 발행
                        let mut detail = Object::new();
                        // currentTime 속성 설정
                        let _ = js_sys::Reflect::set(
                            &detail,
                            &JsValue::from_str("currentTime"),
                            &JsValue::from_f64(time),
                        );
                        // duration 속성 설정
                        let _ = js_sys::Reflect::set(
                            &detail,
                            &JsValue::from_str("duration"),
                            &JsValue::from_f64(self.last_recording_time),
                        );
                        
                        let event = CustomEvent::new_with_event_init_dict(
                            "playbackTimeUpdate",
                            CustomEventInit::new()
                                .bubbles(true)
                                .detail(&detail),
                        ).unwrap();
                        
                        let _ = document.dispatch_event(&event);
                    }
                }
                
                // 현재 재생 시점의 주파수 찾기
                if let Some((closest_t, freqs)) = self.history.iter()
                    .filter(|(t, fs)| (t - time).abs() < 0.2 && !fs.is_empty()) // 시간 허용 오차 설정
                    .min_by(|(t1, _), (t2, _)| {
                        let diff1 = (t1 - time).abs();
                        let diff2 = (t2 - time).abs();
                        diff1.partial_cmp(&diff2).unwrap_or(std::cmp::Ordering::Equal)
                    }) {
                    
                    if !freqs.is_empty() {
                        let current_playback_freq = freqs[0].0;
                        
                        // 현재 주파수 값 업데이트 (PitchPlot에 표시됨)
                        self.current_freq = current_playback_freq;
                        
                        // 주파수에 해당하는 음표명도 업데이트
                        if current_playback_freq > 0.0 {
                            self.pitch = frequency_to_note_octave(current_playback_freq);
                        }
                        
                        web_sys::console::log_1(&format!("🎵 재생 시간 {:.2}s의 주파수: {:.2}Hz ({})", 
                            time, current_playback_freq, self.pitch).into());
                    }
                } else {
                    // 해당 시점에 주파수 데이터가 없으면 0으로 설정 (표시 안 함)
                    self.current_freq = 0.0;
                }
                
                // 현재 재생 시점의 진폭 데이터 찾기
                if let Some((closest_t, amp_data)) = self.amplitude_history.iter()
                    .filter(|(t, _)| (t - time).abs() < 0.2) // 시간 허용 오차 설정
                    .min_by(|(t1, _), (t2, _)| {
                        let diff1 = (t1 - time).abs();
                        let diff2 = (t2 - time).abs();
                        diff1.partial_cmp(&diff2).unwrap_or(std::cmp::Ordering::Equal)
                    }) {
                    
                    // 저장된 진폭 데이터 사용
                    self.amplitude_data = Some(amp_data.clone());
                    
                    // RMS 값도 계산해서 업데이트 (필요한 경우)
                    let rms = (amp_data.iter().map(|&x| x * x).sum::<f32>() / amp_data.len() as f32).sqrt();
                    self.current_rms = rms;
                    
                    // 로그 줄여서 성능 향상
                    if time % 1.0 < 0.03 { // 대략 1초마다 한 번만 로그 출력
                        web_sys::console::log_1(&format!("🔊 재생 시간 {:.2}s의 진폭 데이터: {} 개, RMS: {:.3}", 
                            time, amp_data.len(), rms).into());
                    }
                } else {
                    // 해당 시점에 진폭 데이터가 없으면 빈 데이터 설정
                    let empty_amplitude = vec![0.0f32; 128];
                    self.amplitude_data = Some(empty_amplitude);
                    self.current_rms = 0.0;
                }
                
                // 재생 최대 시간 업데이트 (기록된 history의 마지막 시간값과 비교)
                if let Some((last_time, _)) = self.history.back() {
                    if time > *last_time {
                        // 현재 재생 시간이 기록된 마지막 시간보다 크면 이상 - 로그 출력
                        web_sys::console::log_1(&format!("⚠️ 재생 시간이 기록 범위를 벗어남: {:.2}s > {:.2}s", time, last_time).into());
                    }
                }
                
                // 재생 중 로그 출력
                web_sys::console::log_1(&format!("⏱️ 재생 시간 업데이트: {:.2}s, is_playing: {}", time, self.is_playing).into());
                
                true
            }

            Msg::PlaybackEnded => {
                // 이미 재생 중이 아니면 중복 호출 무시
                if !self.is_playing {
                    web_sys::console::log_1(&"이미 재생이 종료되었습니다".into());
                    return false;
                }
                
                // 재생 완료 로그
                web_sys::console::log_1(&"⏹️ 재생 종료, 재생 상태 초기화".into());
                
                // 인터벌 타이머 제거
                self.playback_interval = None;
                
                // 상태 초기화
                self.is_playing = false;
                
                // 재생 시간을 마지막 녹음 시간으로 설정 (게이지바가 끝까지 가도록)
                if let Some(audio_element) = &self.audio_element {
                    // 재생 요소의 실제 duration을 체크
                    let actual_duration = audio_element.duration();
                    if actual_duration > 0.0 && actual_duration.is_finite() {
                        // 실제 오디오 길이가 last_recording_time과 다르면 업데이트
                        if (actual_duration - self.last_recording_time).abs() > 0.1 {
                            web_sys::console::log_1(&format!("재생 종료시 오디오 길이 업데이트: {:.2}초 -> {:.2}초", 
                                self.last_recording_time, actual_duration).into());
                            self.last_recording_time = actual_duration;
                        }
                    }
                    // 오디오 요소의 playback time도 업데이트
                    audio_element.set_current_time(self.last_recording_time);
                }
                
                // playback_time을 정확히 마지막 녹음 시간으로 설정
                self.playback_time = self.last_recording_time;
                
                // 재생 완료 시 마지막 진폭 데이터 찾기 및 업데이트
                let last_time = self.last_recording_time;
                if let Some((_, amp_data)) = self.amplitude_history.iter()
                    .filter(|(t, _)| *t <= last_time) // 마지막 시간 이전의 데이터
                    .max_by(|(t1, _), (t2, _)| t1.partial_cmp(t2).unwrap_or(std::cmp::Ordering::Equal)) {
                    
                    // 저장된 진폭 데이터 사용
                    self.amplitude_data = Some(amp_data.clone());
                    
                    // RMS 값도 계산해서 업데이트
                    let rms = (amp_data.iter().map(|&x| x * x).sum::<f32>() / amp_data.len() as f32).sqrt();
                    self.current_rms = rms;
                    
                    web_sys::console::log_1(&format!("🔊 재생 완료 시 마지막 진폭 데이터: {} 개, RMS: {:.3}", 
                        amp_data.len(), rms).into());
                }
                
                // 재생 시간 UI 업데이트 (게이지바를 정확히 끝까지 채움)
                self.update_playback_time_ui(self.last_recording_time);
                
                // 재생 상태 이벤트 발행
                if let Some(window) = web_sys::window() {
                    if let Some(document) = window.document() {
                        // 재생 상태 변경 이벤트 발행
                        let event = CustomEvent::new_with_event_init_dict(
                            "playbackStateChange",
                            CustomEventInit::new()
                                .bubbles(true)
                                .detail(&JsValue::from_bool(false)),
                        ).unwrap();
                        let _ = document.dispatch_event(&event);
                        
                        // 재생 종료 이벤트 발행
                        let event = web_sys::Event::new("playbackEnded").unwrap();
                        let _ = document.dispatch_event(&event);
                        web_sys::console::log_1(&"playbackEnded 이벤트 발행".into());
                    }
                }
                
                true
            },
            
            // 새 메시지 추가: 오디오 위치 초기화
            Msg::ResetAudioPosition => {
                // 오디오 요소 위치 초기화
                if let Some(audio_element) = &self.audio_element {
                    audio_element.set_current_time(0.0);
                    self.playback_time = 0.0;
                    web_sys::console::log_1(&"오디오 요소의 위치 초기화됨".into());
                    
                    // UI도 업데이트 (게이지바 위치를 0으로 설정)
                    self.update_playback_time_ui(0.0);
                }
                true
            },

            Msg::RecorderReady(recorder) => {
                // 레코더 객체 저장
                self.recorder = Some(recorder);
                true
            }
            
            // 새로운 메시지 타입 추가: 시크 (재생 위치 변경)
            Msg::SeekPlayback(progress) => {
                if !self.has_recorded_audio() || self.is_recording {
                    return false;
                }
                
                if let Some(audio_element) = &self.audio_element {
                    // 전체 녹음 시간
                    let total_duration = self.last_recording_time;
                    
                    // 진행률을 시간으로 변환
                    let seek_time = progress * total_duration;
                    
                    // 0보다 작거나 총 길이보다 크면 제한
                    let seek_time = seek_time.max(0.0).min(total_duration);
                    
                    // 현재 재생 중인지 상태 저장
                    let was_playing = self.is_playing;
                    
                    // 시크 위치의 시간값 업데이트 (항상 수행)
                    self.playback_time = seek_time;
                    
                    // 현재 시크 위치의 주파수 정보 검색 및 업데이트
                    if let Some((_, freqs)) = self.history.iter()
                        .filter(|(t, fs)| (t - seek_time).abs() < 0.2 && !fs.is_empty()) // 0.2초 내의 데이터 중 주파수가 있는 것
                        .min_by(|(t1, _), (t2, _)| {
                            let diff1 = (t1 - seek_time).abs();
                            let diff2 = (t2 - seek_time).abs();
                            diff1.partial_cmp(&diff2).unwrap_or(std::cmp::Ordering::Equal)
                        }) {
                        
                        // 가장 강한 주파수 (첫 번째 요소)로 현재 주파수 업데이트
                        if !freqs.is_empty() {
                            let strongest_freq = freqs[0].0;
                            self.current_freq = strongest_freq;
                            
                            if strongest_freq > 0.0 {
                                self.pitch = frequency_to_note_octave(strongest_freq);
                                web_sys::console::log_1(&format!("🎵 시크 위치의 주파수: {:.2}Hz ({})", 
                                    strongest_freq, self.pitch).into());
                            }
                        }
                    }
                    
                    // 현재 시크 위치의 진폭 데이터 검색 및 업데이트
                    if let Some((_, amp_data)) = self.amplitude_history.iter()
                        .filter(|(t, _)| (t - seek_time).abs() < 0.2) // 0.2초 내의 데이터
                        .min_by(|(t1, _), (t2, _)| {
                            let diff1 = (t1 - seek_time).abs();
                            let diff2 = (t2 - seek_time).abs();
                            diff1.partial_cmp(&diff2).unwrap_or(std::cmp::Ordering::Equal)
                        }) {
                        
                        // 저장된 진폭 데이터 사용
                        self.amplitude_data = Some(amp_data.clone());
                        
                        // RMS 값도 계산해서 업데이트 (필요한 경우)
                        let rms = (amp_data.iter().map(|&x| x * x).sum::<f32>() / amp_data.len() as f32).sqrt();
                        self.current_rms = rms;
                        
                        web_sys::console::log_1(&format!("🔊 시크 위치의 진폭 데이터: {} 개, RMS: {:.3}", 
                            amp_data.len(), rms).into());
                    } else {
                        // 해당 시점에 진폭 데이터가 없으면 빈 데이터 설정
                        let empty_amplitude = vec![0.0f32; 128];
                        self.amplitude_data = Some(empty_amplitude);
                        self.current_rms = 0.0;
                    }
                    
                    // UI 시간 업데이트 (항상 수행)
                    self.update_playback_time_ui(seek_time);
                    
                    // 재생 중인 경우에만 오디오 요소의 재생 위치 변경 및 재생 상태 유지
                    if was_playing {
                        // 일시 중지
                        if let Err(err) = audio_element.pause() {
                            web_sys::console::error_1(&format!("시크 전 일시 중지 실패: {:?}", err).into());
                        }
                        
                        // 오디오 요소의 재생 위치 변경
                        audio_element.set_current_time(seek_time);
                        
                        web_sys::console::log_1(&format!("🎯 재생 위치 변경: {:.2}초 ({:.1}%)", 
                            seek_time, progress * 100.0).into());
                        
                        // 재생 시작
                        if let Err(err) = audio_element.play() {
                            web_sys::console::error_1(&format!("시크 후 재생 시작 실패: {:?}", err).into());
                        } else {
                            // 재생 상태 유지
                            
                            // 재생 인터벌이 없으면 다시 설정
                            if self.playback_interval.is_none() {
                                let link = ctx.link().clone();
                                let audio_element_clone = audio_element.clone();
                                
                                // 새 인터벌 생성
                                let interval = gloo::timers::callback::Interval::new(100, move || {
                                    // 오디오 요소가 아직 유효한지 확인
                                    if audio_element_clone.ended() {
                                        web_sys::console::log_1(&"재생 종료 감지됨 (인터벌)".into());
                                        link.send_message(Msg::PlaybackEnded);
                                        return;
                                    }
                                    
                                    // 현재 재생 시간 가져오기
                                    let current_time = audio_element_clone.current_time();
                                    
                                    // 시간 업데이트 메시지 전송 - 모든 시간값 전송
                                    link.send_message(Msg::UpdatePlaybackTime(current_time));
                                });
                                
                                // 인터벌 핸들 저장
                                self.playback_interval = Some(interval);
                            }
                        }
                    } else {
                        // 일시정지 상태에서는 오디오 요소의 currentTime만 업데이트하고, 재생은 시작하지 않음
                        audio_element.set_current_time(seek_time);
                        web_sys::console::log_1(&format!("🎯 재생 위치만 변경: {:.2}초 ({:.1}%)", 
                            seek_time, progress * 100.0).into());
                    }
                    
                    true
                } else {
                    web_sys::console::error_1(&"시크할 오디오 요소가 없음".into());
                    false
                }
            }

            Msg::UpdateRecordingDuration(actual_duration) => {
                // 실제 오디오 길이 검증 (비정상적으로 큰 값이나 작은 값은 거부)
                if actual_duration <= 0.0 || actual_duration > 3600.0 {
                    web_sys::console::error_1(&format!("비정상적인 오디오 길이 감지됨: {:.2}초, 무시함", actual_duration).into());
                    return false;
                }
                
                // 실제 오디오 길이가 기록된 길이와 차이가 나면 업데이트
                if (actual_duration - self.last_recording_time).abs() > 0.1 {
                    web_sys::console::log_1(&format!("녹음 길이 업데이트: {:.2}초 -> {:.2}초", 
                        self.last_recording_time, actual_duration).into());
                    
                    // 이전 녹음 시간 저장
                    let previous_recording_time = self.last_recording_time;
                    
                    // 마지막 녹음 시간 업데이트
                    self.last_recording_time = actual_duration;
                    
                    // 현재 재생 위치와 최종 녹음 시간의 비율 계산 (진행률)
                    let current_progress = if previous_recording_time > 0.0 {
                        self.playback_time / previous_recording_time
                    } else {
                        0.0
                    };
                    
                    // 재생 중이 아닐 때 재생 위치가 끝에 있었다면(0.9 이상), 
                    // 새 녹음 길이 기준으로도 끝에 있도록 조정
                    if !self.is_playing && current_progress > 0.9 {
                        self.playback_time = actual_duration;
                        web_sys::console::log_1(&format!("재생 위치 끝으로 조정: {:.2}초", actual_duration).into());
                    }
                    
                    // UI 업데이트 - 진행률이 유지되도록 보정
                    self.update_playback_time_ui(self.playback_time);
                    
                    // 재생 종료 상태에서 실제 게이지 위치 강제 업데이트 
                    // (이미 재생이 끝났지만 게이지가 끝에 있지 않은 경우)
                    if let Some(audio_element) = &self.audio_element {
                        if audio_element.ended() {
                            // 재생이 끝난 상태면 게이지를 끝으로 조정
                            self.playback_time = actual_duration;
                            self.update_playback_time_ui(actual_duration);
                            web_sys::console::log_1(&"재생 완료 상태: 게이지 위치를 끝으로 보정".into());
                        }
                    }
                }
                true
            },

            // 새 메시지 추가: 오디오 리소스 정리
            Msg::StopAudioResources => {
                // 오디오 컨텍스트가 있으면 정지
                if let Some(ctx) = &self.audio_ctx {
                    let _ = ctx.close();
                }

                // 스트림 트랙 정지
                if let Some(stream) = &self._stream {
                    let tracks = stream.get_audio_tracks();
                    for i in 0..tracks.length() {
                        let track_js = tracks.get(i);
                        let track = web_sys::MediaStreamTrack::from(track_js);
                        track.stop();
                    }
                }

                // 상태 초기화
                self.audio_ctx = None;
                self.analyser = None;
                self._stream = None;
                self.mic_active = false;
                self.pitch = "🎤 음성 입력 대기...".to_string();
                self.prev_freqs.clear();
                self.current_freq = 0.0;

                // 인터벌 정리
                self.playback_interval = None;
                self.analysis_interval = None;
                
                // 최대 녹음 시간 타이머 취소
                self.max_recording_timer = None;

                // 컨트롤 버튼 활성화 이벤트 발생
                if let Some(window) = web_sys::window() {
                    if let Some(document) = window.document() {
                        // 이벤트 생성 및 발생
                        let enable_event = web_sys::Event::new("enableControlButtons").expect("enableControlButtons 이벤트 생성 실패");
                        if let Err(err) = document.dispatch_event(&enable_event) {
                            web_sys::console::error_1(&format!("enableControlButtons 이벤트 발생 실패: {:?}", err).into());
                        } else {
                            web_sys::console::log_1(&"컨트롤 버튼 활성화 이벤트 발생 성공 (StopAudioResources)".into());
                        }
                    }
                }

                web_sys::console::log_1(&"오디오 리소스 및 모든 인터벌 중지됨".into());

                true
            },

            Msg::DownloadRecording => {
                // 녹음된 오디오가 없으면 다운로드 불가
                if !self.has_recorded_audio() {
                    web_sys::console::log_1(&"다운로드할 녹음된 오디오가 없습니다".into());
                    return false;
                }
                
                // 오디오 URL로부터 다운로드 진행
                if let Some(audio_url) = &self.recorded_audio_url {
                    // 파일명 생성 (녹음 생성 시간 기반으로 한국어 형식 포맷)
                    let date = js_sys::Date::new(&JsValue::from_f64(self.created_at_time));
                    
                    // 한국어 날짜 형식: YYYY-MM-DD_HH-MM-SS
                    let year = date.get_full_year();
                    let month = date.get_month() + 1; // 월은 0부터 시작하므로 +1
                    let day = date.get_date();
                    let hours = date.get_hours();
                    let minutes = date.get_minutes();
                    let seconds = date.get_seconds();
                    
                    let filename = format!(
                        "recording_{:04}-{:02}-{:02}_{:02}-{:02}-{:02}.webm",
                        year, month, day, hours, minutes, seconds
                    );

                    if let Some(window) = web_sys::window() {
                        if let Some(document) = window.document() {
                            if let Ok(element) = document.create_element("a") {
                                let a_element: web_sys::HtmlAnchorElement = element
                                    .dyn_into()
                                    .expect("a 태그 생성 실패");
                                
                                // 오디오 URL 복제본 생성 (메타데이터 유지)
                                a_element.set_href(audio_url);
                                
                                // 다운로드 속성 설정
                                a_element.set_attribute("download", &filename).unwrap_or_else(|_| {
                                    web_sys::console::error_1(&"download 속성 설정 실패".into());
                                });
                                
                                // 다운로드 시작 (DOM에 추가하고 클릭 후 제거)
                                document.body().unwrap().append_child(&a_element).unwrap();
                                a_element.click();
                                document.body().unwrap().remove_child(&a_element).unwrap();
                                
                                web_sys::console::log_1(&format!("오디오 다운로드 완료: {}", filename).into());
                                
                                return true;
                            }
                        }
                    }
                }
                
                web_sys::console::error_1(&"오디오 다운로드 실패".into());
                false
            },
            
            // 새 메시지 추가: 컴포넌트 상태 완전 초기화
            Msg::ResetComponent => {
                web_sys::console::log_1(&"PitchAnalyzer 컴포넌트 상태 초기화 시작".into());
                
                // 오디오 재생/녹음 관련 상태 초기화
                if self.is_playing {
                    if let Some(audio_element) = &self.audio_element {
                        let _ = audio_element.pause();
                    }
                    self.is_playing = false;
                }
                
                // 녹음 중이면 중지
                if self.is_recording {
                    if let Some(recorder) = &self.recorder {
                        if recorder.state() == web_sys::RecordingState::Recording {
                            let _ = recorder.stop();
                        }
                    }
                    self.is_recording = false;
                }
                
                // 오디오 컨텍스트 정리
                if let Some(ctx) = &self.audio_ctx {
                    let _ = ctx.close();
                }
                
                // 스트림 트랙 정지
                if let Some(stream) = &self._stream {
                    let tracks = stream.get_audio_tracks();
                    for i in 0..tracks.length() {
                        let track_js = tracks.get(i);
                        let track = web_sys::MediaStreamTrack::from(track_js);
                        track.stop();
                    }
                }
                
                // URL 리소스 정리
                if let Some(url) = &self.recorded_audio_url {
                    let _ = web_sys::Url::revoke_object_url(url);
                }
                
                // 모든 인터벌 및 타이머 정리
                self.analysis_interval = None;
                self.playback_interval = None;
                self.max_recording_timer = None;
                
                // 오디오 요소 이벤트 핸들러 제거
                if let Some(audio) = &self.audio_element {
                    audio.set_onloadeddata(None);
                    audio.set_onloadedmetadata(None);
                    audio.set_onended(None);
                }
                
                // 레코더 이벤트 핸들러 제거
                if let Some(recorder) = &self.recorder {
                    recorder.set_ondataavailable(None);
                    recorder.set_onstop(None);
                }
                
                // 스피커 노드 연결 해제
                if let Some(speaker_node) = &self.speaker_node {
                    speaker_node.disconnect();
                }
                
                // 모든 데이터 컬렉션 비우기
                self.prev_freqs.clear();
                self.history.clear();
                self.recorded_chunks.clear();
                
                // 기본 상태로 재설정
                self.audio_ctx = None;
                self.analyser = None;
                self._stream = None;
                self.pitch = "🎤 음성 입력 대기...".to_string();
                self.current_freq = 0.0;
                self.elapsed_time = 0.0;
                self.mic_active = false;
                self.monitor_active = false;
                self.speaker_node = None;
                self.recorder = None;
                
                // DOM에서 오디오 요소 제거
                if let Some(window) = web_sys::window() {
                    if let Some(document) = window.document() {
                        if let Some(audio_element) = document.get_element_by_id("pitch-analyzer-audio") {
                            if let Some(parent) = audio_element.parent_node() {
                                let _ = parent.remove_child(&audio_element);
                                web_sys::console::log_1(&"DOM에서 오디오 요소 제거됨".into());
                            }
                        }
                    }
                }
                
                self.recorded_audio_url = None;
                self.audio_element = None;
                self.playback_time = 0.0;
                self.last_recording_time = 0.0;
                self.recording_start_time = 0.0;
                self.is_frozen = false;
                self.created_at_time = js_sys::Date::new_0().get_time();
                
                // 감도는 기본값으로 유지 (props 설정 유지를 위함)
                // self.sensitivity = 0.01;
                // self.show_links는 props로부터 온 값이므로 변경하지 않음
                
                // 진폭 데이터 초기화
                self.amplitude_data = None;
                self.amplitude_history.clear();
                self.current_rms = 0.0;
                
                web_sys::console::log_1(&"PitchAnalyzer 컴포넌트 상태 초기화 완료".into());
                
                true
            },
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let current_freq = if self.is_playing {
            // 재생 중일 때, history에서 현재 playback_time에 가장 가까운 주파수 찾기
            let playback_t = self.playback_time;
            let closest_data = self.history.iter()
                .min_by(|(t1, _), (t2, _)| {
                    let diff1 = (t1 - playback_t).abs();
                    let diff2 = (t2 - playback_t).abs();
                    diff1.partial_cmp(&diff2).unwrap_or(std::cmp::Ordering::Equal)
                });
            
            if let Some((_, freqs)) = closest_data {
                if !freqs.is_empty() {
                    // 가장 강한 주파수(첫 번째 요소) 반환
                    freqs[0].0
                } else {
                    self.current_freq
                }
            } else {
                self.current_freq
            }
        } else {
            self.current_freq
        };

        let history = VecDeque::from(self.history.clone().into_iter().collect::<Vec<_>>());
        let show_links = self.show_links;
        let playback_time = if self.is_recording {
            // 녹음 중에는 재생 시간을 전달하지 않음
            None
        } else {
            Some(self.playback_time)
        };
        let is_playing = self.is_playing;
        let is_recording = self.is_recording;
        let is_frozen = self.is_frozen;

        // 피치 플롯 컴포넌트
        let pitch_plot = html! {
            <PitchPlot 
                current_freq={current_freq} 
                history={history} 
                playback_time={playback_time}
                is_playing={is_playing}
                is_recording={is_recording}
                is_frozen={is_frozen}
            />
        };

        // 진폭 시각화 컴포넌트
        let amplitude_visualizer = html! {
            <AmplitudeVisualizer 
                amplitude_data={self.amplitude_data.clone()}
                sample_rate={Some(44100.0)}
                is_recording={self.is_recording}
                is_playing={self.is_playing}
                history={Some(self.amplitude_history.clone())}
            />
        };
        
        // 메트로놈 컴포넌트
        let metronome = html! {
            <Metronome />
        };
        
        // 스케일 생성기 컴포넌트
        let scale_generator = html! {
            <ScaleGenerator />
        };

        // 피아노 컴포넌트
        let piano = html! {
            <Piano />
        };

        // show_links 속성을 확인하여 dashboard 스타일 또는 직접 렌더링 결정
        if ctx.props().show_links.unwrap_or(true) {
            // 대시보드 레이아웃 구성 (메인 페이지)
            let items = vec![
                DashboardItem {
                    id: "pitch-plot".to_string(),
                    component: pitch_plot,
                    width: 3,
                    height: 3,
                    route: Some(Route::PitchPlot),
                    show_link: self.show_links,
                    aspect_ratio: 16.0/9.0,
                    custom_style: Some("height: 100%; width: 100%;".to_string()),
                },
                DashboardItem {
                    id: "amplitude-visualizer".to_string(),
                    component: amplitude_visualizer,
                    width: 1,
                    height: 1,
                    route: Some(Route::AmplitudeVisualizer),
                    show_link: self.show_links,
                    aspect_ratio: 16.0/9.0,
                    custom_style: None,
                },
                DashboardItem {
                    id: "metronome".to_string(),
                    component: metronome,
                    width: 1,
                    height: 1,
                    route: Some(Route::Metronome),
                    show_link: self.show_links,
                    aspect_ratio: 16.0/9.0,
                    custom_style: None,
                },
                DashboardItem {
                    id: "scale-generator".to_string(),
                    component: scale_generator,
                    width: 2,
                    height: 2,
                    route: Some(Route::ScaleGenerator),
                    show_link: self.show_links,
                    aspect_ratio: 16.0/9.0,
                    custom_style: None,
                },
                DashboardItem {
                    id: "piano-keyboard".to_string(),
                    component: piano,
                    width: 5,
                    height: 1,
                    route: Some(Route::PianoKeyboard),
                    show_link: self.show_links,
                    aspect_ratio: 26.7/3.0,
                    custom_style: None,
                },
            ];

            let layout = DashboardLayout { items, columns: 5 };

            html! {
                <div class="app-container">
                    <Dashboard layout={layout} />
                </div>
            }
        } else {
            // 직접 렌더링 (상세 페이지)
            // 현재 라우트에 따라 해당 컴포넌트만 렌더링
            let current_route = if let Some(window) = web_sys::window() {
                if let Some(location) = window.location().pathname().ok() {
                    if location.contains("amplitude") {
                        "amplitude"
                    } else if location.contains("metronome") {
                        "metronome"
                    } else if location.contains("scale-generator") {
                        "scale-generator"
                    } else if location.contains("piano-keyboard") {
                        "piano-keyboard"
                    } else {
                        "pitch"
                    }
                } else {
                    "pitch"
                }
            } else {
                "pitch"
            };

            html! {
                <div class="pitch-analyzer-direct">
                    {
                        if current_route == "amplitude" {
                            amplitude_visualizer
                        } else if current_route == "metronome" {
                            metronome
                        } else if current_route == "scale-generator" {
                            scale_generator
                        } else if current_route == "piano-keyboard" {
                            html! { <Piano /> }
                        } else {
                            pitch_plot
                        }
                    }
                </div>
            }
        }
    }
}

// Yew 앱 진입점
#[function_component(App)]
fn app() -> Html {
    html! {
        <BrowserRouter>
            <Switch<Route> render={switch} />
        </BrowserRouter>
    }
}

// main 함수 정의 (wasm 앱 진입점)
fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<App>::new().render();
}
