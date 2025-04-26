use crate::dashboard::{Dashboard, DashboardItem, DashboardLayout};
use crate::pitch_plot::PitchPlot;
use crate::routes::{switch, Route};
use gloo::events::EventListener;
use js_sys::{Float32Array, Promise};
use log::info;
use std::collections::VecDeque;
use std::f64::consts::PI;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AnalyserNode, AudioContext, HtmlCanvasElement, MediaDevices, MediaStream,
    MediaStreamAudioSourceNode, MediaStreamConstraints, Navigator,
};
use yew::prelude::*;
use yew_router::prelude::*;

mod dashboard;
mod pitch_plot;
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
    
    // 재생 관련 메시지
    TogglePlayback,          // 재생/일시정지 토글
    StartPlayback,           // 재생 시작
    PausePlayback,           // 재생 일시정지
    UpdatePlaybackTime(f64), // 재생 시간 업데이트
    PlaybackEnded,           // 재생 완료
    RecorderReady(web_sys::MediaRecorder), // 새로 추가된 메시지 타입
}

impl Component for PitchAnalyzer {
    type Message = Msg;
    type Properties = ();

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
        
        playback_listener.forget();

        toggle_audio_listener.forget();
        sensitivity_listener.forget();
        toggle_listener.forget();
        monitor_listener.forget();
        volume_listener.forget();

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
            show_links: true,      // 기본적으로 링크 표시
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
                                    // 녹음이 중지되면 명시적으로 StopRecording 메시지 보내서 모든 이벤트 리스너를 제거
                                    link_clone.send_message(Msg::StopRecording);
                                }) as Box<dyn FnMut(web_sys::Event)>);
                                
                                recorder.set_ondataavailable(Some(ondataavailable.as_ref().unchecked_ref()));
                                recorder.set_onstop(Some(onstop.as_ref().unchecked_ref()));
                                
                                // 이벤트 핸들러 메모리 릭 방지를 위해 forget 호출
                                ondataavailable.forget();
                                onstop.forget();
                                
                                // 100ms 간격으로 데이터 수집하도록 설정
                                if let Err(err) = recorder.start_with_time_slice(100) {
                                    web_sys::console::error_1(&format!("Failed to start recorder: {:?}", err).into());
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

                        // 현재 상대 시간과 함께 주파수 목록 기록
                        self.history.push_back((current_time, freqs));
                        
                        // 로그 출력 (디버깅용)
                        web_sys::console::log_1(&format!("🕒 녹음 경과 시간: {:.2}s, 주파수: {:.2}Hz", current_time, average_freq).into());
                    } else {
                        self.pitch = "🔇 너무 작은 소리 (무시됨)".to_string();
                        self.prev_freqs.clear();
                        self.current_freq = 0.0;

                        // 빈 주파수 목록 기록 (시간은 계속 유지)
                        self.history.push_back((current_time, Vec::new()));
                    }
                    
                    // 외부 참조용 시간 업데이트
                    self.elapsed_time = current_time;

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
                gloo::timers::callback::Interval::new(100, move || {
                    link.send_message(Msg::UpdatePitch);
                })
                .forget();

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
                    if let Some(recorder) = &self.recorder {
                        if recorder.state() == web_sys::RecordingState::Recording {
                            recorder.stop().expect("Failed to stop recording");
                        }
                    }
                    self.is_recording = false;
                    self.last_recording_time = self.elapsed_time;
                    
                    // 녹음된 청크를 결합하여 URL 생성
                    let blobs = js_sys::Array::new();
                    for blob in &self.recorded_chunks {
                        blobs.push(blob);
                    }
                    
                    if !self.recorded_chunks.is_empty() {
                        // Blob 배열을 하나의 Blob으로 합치기
                        let mut blob_options = web_sys::BlobPropertyBag::new();
                        blob_options.type_("audio/webm");
                        
                        if let Ok(combined_blob) = web_sys::Blob::new_with_blob_sequence_and_options(&blobs, &blob_options) {
                            // Blob URL 생성
                            let url = web_sys::Url::create_object_url_with_blob(&combined_blob)
                                .expect("Failed to create object URL");
                            
                            self.recorded_audio_url = Some(url.clone());
                            info!("url: {:?}", self.recorded_audio_url);
                            
                            // 오디오 요소 생성
                            if let Some(window) = web_sys::window() {
                                if let Some(document) = window.document() {
                                    if let Ok(element) = document.create_element("audio") {
                                        let audio_element: web_sys::HtmlAudioElement = element
                                            .dyn_into()
                                            .expect("Failed to create audio element");
                                        
                                        // 기존 오디오 요소가 있으면 이벤트 리스너 제거 및 리소스 정리
                                        if let Some(old_audio) = &self.audio_element {
                                            old_audio.set_onloadeddata(None);
                                            old_audio.set_onended(None);
                                            
                                            // URL 리소스 정리
                                            if let Some(old_url) = &self.recorded_audio_url {
                                                let _ = web_sys::Url::revoke_object_url(old_url);
                                            }
                                        }
                                        
                                        audio_element.set_src(&url);
                                        audio_element.set_controls(false);
                                        
                                        // 로드 완료 이벤트 리스너 추가
                                        let link = ctx.link().clone();
                                        let onloadeddata = Closure::wrap(Box::new(move |_: web_sys::Event| {
                                            web_sys::console::log_1(&"새 오디오 데이터 로드 완료".into());
                                        }) as Box<dyn FnMut(web_sys::Event)>);
                                        audio_element.set_onloadeddata(Some(onloadeddata.as_ref().unchecked_ref()));
                                        onloadeddata.forget();
                                        
                                        self.audio_element = Some(audio_element);
                                    }
                                }
                            }
                        }
                    }
                    
                    // 녹음 데이터 초기화
                    self.recorded_chunks.clear();
                }

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

                true
            }

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
                
                web_sys::console::log_1(&"녹음 시작: 시간 초기화됨".into());

                true
            }

            Msg::StopRecording => {
                // 이미 녹음 중지 상태면 무시
                if !self.is_recording {
                    return false;
                }
                
                self.is_recording = false;
                
                // MediaRecorder 정리
                if let Some(recorder) = &self.recorder {
                    // 데이터 이벤트 핸들러 제거
                    recorder.set_ondataavailable(None);
                    recorder.set_onstop(None);
                    
                    // 레코더가 아직 녹음 중이면 중지
                    if recorder.state() == web_sys::RecordingState::Recording {
                        let _ = recorder.stop(); // 에러는 무시
                    }
                    
                    web_sys::console::log_1(&"Recording stopped and recorder event listeners removed".into());
                }
                
                // 모든 관련 상태 초기화
                self.recorder = None;
                
                // 마지막 녹음 시간 저장
                self.last_recording_time = self.elapsed_time;
                web_sys::console::log_1(&format!("녹음 종료: 총 녹음 시간 = {:.2}초", self.last_recording_time).into());
                
                true
            }

            Msg::RecordingDataAvailable(blob) => {
                // 녹음 중일 때만 데이터 추가
                if self.is_recording && blob.size() > 0.0 {
                    self.recorded_chunks.push(blob);
                    info!("blob: {:?}", self.recorded_chunks.len());
                } else {
                    // 녹음 중이 아니면 데이터 무시
                    web_sys::console::log_1(&"Ignoring data chunk - not recording".into());
                }
                true
            }

            Msg::RecordingComplete(url) => {
                // 녹음 완료
                self.is_recording = false;
                self.recorded_audio_url = Some(url.clone());
                
                // 오디오 요소 생성
                if let Some(window) = web_sys::window() {
                    if let Some(document) = window.document() {
                        if let Ok(element) = document.create_element("audio") {
                            let audio_element: web_sys::HtmlAudioElement = element
                                .dyn_into()
                                .expect("Failed to create audio element");
                            
                            audio_element.set_src(&url);
                            audio_element.set_controls(false);
                            
                            // 재생 종료 이벤트 리스너 추가
                            let link = ctx.link().clone();
                            let onended = Closure::wrap(Box::new(move |_: web_sys::Event| {
                                link.send_message(Msg::PlaybackEnded);
                            }) as Box<dyn FnMut(web_sys::Event)>);
                            
                            // 로드 완료 이벤트 리스너 추가
                            let link_load = ctx.link().clone();
                            let onloadeddata = Closure::wrap(Box::new(move |_: web_sys::Event| {
                                web_sys::console::log_1(&"Audio data loaded successfully".into());
                            }) as Box<dyn FnMut(web_sys::Event)>);
                            
                            audio_element.set_onended(Some(onended.as_ref().unchecked_ref()));
                            audio_element.set_onloadeddata(Some(onloadeddata.as_ref().unchecked_ref()));
                            onended.forget();
                            onloadeddata.forget();
                            
                            self.audio_element = Some(audio_element);
                        }
                    }
                }
                
                true
            }

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
                    
                    // 재생 전에 명시적으로 currentTime을 0으로 리셋
                    audio_element.set_current_time(0.0);
                    self.playback_time = 0.0;
                    
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
                    
                    // 재생 상태 업데이트를 위한 인터벌 설정
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
                self.playback_time = 0.0;
                
                true
            }

            Msg::RecorderReady(recorder) => {
                // 레코더 객체 저장
                self.recorder = Some(recorder);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let current_freq = self.current_freq;
        let history = VecDeque::from(self.history.clone().into_iter().collect::<Vec<_>>());
        let show_links = self.show_links;
        let playback_time = if self.is_playing { Some(self.playback_time) } else { None };
        let is_playing = self.is_playing;

        // 피치 플롯 컴포넌트
        let pitch_plot = html! {
            <PitchPlot 
                current_freq={current_freq} 
                history={history} 
                playback_time={playback_time}
                is_playing={is_playing}
            />
        };

        // 대시보드 레이아웃 구성
        let items = vec![DashboardItem {
            id: "pitch-plot".to_string(),
            component: pitch_plot,
            width: 2,
            height: 2,
            route: Some(Route::PitchPlot),
            show_link: self.show_links,
        }];

        let layout = DashboardLayout { items, columns: 3 };

        html! {
            <div class="app-container">
                <Dashboard layout={layout} />
            </div>
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
