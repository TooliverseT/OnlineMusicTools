use crate::pitch_plot::PitchPlot;
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

mod pitch_plot;

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

    if midi_number < 12 || midi_number > 95 {
        return "Out of range".to_string(); // C0 ~ B6에 해당
    }

    let note = notes[(midi_number % 12) as usize];
    let octave = midi_number / 12 - 1;

    format!("{}{}", note, octave)
}

fn analyze_pitch_autocorrelation(buffer: &[f32], sample_rate: f64) -> Option<f64> {
    const RMS_THRESHOLD: f32 = 0.01;
    const MIN_FREQ: f64 = 50.0;
    const MAX_FREQ: f64 = 1000.0;

    let rms = (buffer.iter().map(|&x| x * x).sum::<f32>() / buffer.len() as f32).sqrt();
    if rms < RMS_THRESHOLD {
        return None;
    }

    let min_lag = (sample_rate / MAX_FREQ) as usize; // 44100 / 1000 = 44
    let max_lag = (sample_rate / MIN_FREQ) as usize; // 44100 / 50 = 882

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

// 🎤 실시간 피치 분석기
pub struct PitchAnalyzer {
    audio_ctx: Option<AudioContext>,
    analyser: Option<AnalyserNode>,
    _stream: Option<MediaStream>,
    pitch: String,
    prev_freqs: VecDeque<f64>,
    history: VecDeque<(f64, f64)>,
    canvas_ref: NodeRef,
    elapsed_time: f64,
    current_freq: f64, // 🔥 현재 주파수
}

pub enum Msg {
    StartAudio,
    UpdatePitch,
    AudioReady(AudioContext, AnalyserNode, MediaStream),
}

impl Component for PitchAnalyzer {
    type Message = Msg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
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
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::StartAudio => {
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

                    if let Some(freq) = analyze_pitch_autocorrelation(&buffer, sample_rate) {
                        if self.prev_freqs.len() >= 5 {
                            self.prev_freqs.pop_front();
                        }
                        self.prev_freqs.push_back(freq);
                        let average_freq =
                            self.prev_freqs.iter().sum::<f64>() / self.prev_freqs.len() as f64;
                        self.current_freq = average_freq;

                        let note = frequency_to_note_octave(average_freq);
                        self.pitch = format!("🎶 현재 음: {} ({:.2} Hz)", note, average_freq);

                        self.history.push_back((self.elapsed_time, average_freq));
                    } else {
                        self.pitch = "🔇 너무 작은 소리 (무시됨)".to_string();
                        self.prev_freqs.clear();
                        self.current_freq = 0.0;

                        // 💡 frequency가 없을 때도 0.0을 기록
                        self.history.push_back((self.elapsed_time, 0.0));
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

                let link = ctx.link().clone();
                gloo::timers::callback::Interval::new(100, move || {
                    link.send_message(Msg::UpdatePitch);
                })
                .forget();

                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <div>
                <h1>{ "🎵 실시간 피치 분석기" }</h1>
                <button onclick={ctx.link().callback(|_| Msg::StartAudio)}>{ "🎤 마이크 시작" }</button>
                <p>{ &self.pitch }</p>
                <PitchPlot current_freq={self.current_freq} history={VecDeque::from(self.history.clone().into_iter().collect::<Vec<_>>())} />
            </div>
        }
    }
}

// Yew 앱 진입점
#[function_component(App)]
fn app() -> Html {
    html! { <PitchAnalyzer /> }
}

// main 함수 정의 (wasm 앱 진입점)
fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<App>::new().render();
}
