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
    speaker_node: Option<web_sys::AudioNode>, // 스피커 출력용 노드
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

        toggle_audio_listener.forget();
        sensitivity_listener.forget();
        toggle_listener.forget();
        monitor_listener.forget();

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
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::StartAudio => {
                // 이미 활성화된 경우 무시
                if self.mic_active {
                    return false;
                }

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
                            link.send_message(Msg::AudioReady(audio_ctx, analyser, stream));
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

                    self.elapsed_time += 0.1;

                    // 여러 주파수 분석
                    let freqs =
                        analyze_multiple_frequencies(&buffer, sample_rate, self.sensitivity);

                    if !freqs.is_empty() {
                        // 가장 강한 주파수 (첫 번째 요소)
                        let strongest_freq = freqs[0].0;

                        // 평균 계산을 위해 이전 목록에 추가
                        if self.prev_freqs.len() >= 5 {
                            self.prev_freqs.pop_front();
                        }
                        self.prev_freqs.push_back(strongest_freq);
                        let average_freq =
                            self.prev_freqs.iter().sum::<f64>() / self.prev_freqs.len() as f64;
                        self.current_freq = average_freq;

                        let note = frequency_to_note_octave(average_freq);
                        self.pitch = format!("🎶 현재 음: {} ({:.2} Hz)", note, average_freq);

                        // 전체 주파수 목록 기록
                        self.history.push_back((self.elapsed_time, freqs));
                    } else {
                        self.pitch = "🔇 너무 작은 소리 (무시됨)".to_string();
                        self.prev_freqs.clear();
                        self.current_freq = 0.0;

                        // 빈 주파수 목록 기록
                        self.history.push_back((self.elapsed_time, Vec::new()));
                    }

                    true
                } else {
                    false
                }
            }

            Msg::AudioReady(audio_ctx, analyser, stream) => {
                self.audio_ctx = Some(audio_ctx);
                self.analyser = Some(analyser);
                self._stream = Some(stream);
                self.mic_active = true;

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

                true
            }

            Msg::ToggleAudio => {
                if self.mic_active {
                    // 마이크가 활성화된 상태면 비활성화
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
                                    // 게인 노드 생성
                                    match audio_ctx.clone().create_gain() {
                                        Ok(gain_node) => {
                                            // 볼륨 설정 (마이크 피드백 방지를 위해 낮게 설정)
                                            let gain_param = gain_node.gain();
                                            gain_param.set_value(0.5);

                                            // 소스를 게인 노드에 직접 연결
                                            if monitor_source
                                                .connect_with_audio_node(&gain_node)
                                                .is_err()
                                            {
                                                web_sys::console::log_1(
                                                    &"Failed to connect source to gain node".into(),
                                                );
                                                self.monitor_active = false;
                                                return false;
                                            }

                                            // 게인 노드를 출력에 연결
                                            if gain_node
                                                .connect_with_audio_node(
                                                    &audio_ctx.clone().destination(),
                                                )
                                                .is_err()
                                            {
                                                web_sys::console::log_1(
                                                    &"Failed to connect gain node to destination"
                                                        .into(),
                                                );
                                                self.monitor_active = false;
                                                return false;
                                            }

                                            // 스피커 노드 저장
                                            self.speaker_node = Some(gain_node.into());
                                            web_sys::console::log_1(
                                                &"Monitor activated with separate source".into(),
                                            );
                                        }
                                        Err(_) => {
                                            web_sys::console::log_1(
                                                &"Failed to create gain node".into(),
                                            );
                                            self.monitor_active = false;
                                            return false;
                                        }
                                    }
                                }
                                Err(_) => {
                                    web_sys::console::log_1(
                                        &"Failed to create monitor source".into(),
                                    );
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
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let current_freq = self.current_freq;
        let history = VecDeque::from(self.history.clone().into_iter().collect::<Vec<_>>());
        let show_links = self.show_links;

        // 피치 플롯 컴포넌트
        let pitch_plot = html! {
            <PitchPlot current_freq={current_freq} history={history} />
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
