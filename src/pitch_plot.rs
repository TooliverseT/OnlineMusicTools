use log::info;
use plotters::prelude::*;
use plotters_canvas::CanvasBackend;
use std::collections::VecDeque;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct PitchPlotProps {
    pub current_freq: f64,
    pub history: VecDeque<(f64, f64)>, // (timestamp, frequency)
}

#[function_component(PitchPlot)]
pub fn pitch_plot(props: &PitchPlotProps) -> Html {
    let canvas_ref = use_node_ref();
    let last_center_midi = use_state(|| 69); // MIDI 69 (A4)를 기본값으로 설정

    {
        let canvas_ref = canvas_ref.clone();
        let history = props.history.clone();
        let current_freq = props.current_freq;
        let last_center_midi_handle = last_center_midi.clone();
        info!("current_freq: {:?}", current_freq);

        use_effect_with((history.clone(), current_freq), move |_| {
            if let Some(canvas) = canvas_ref.cast::<web_sys::HtmlCanvasElement>() {
                let backend = CanvasBackend::with_canvas_object(canvas).unwrap();
                let root = backend.into_drawing_area();
                root.fill(&WHITE).unwrap();

                let (_width, _height) = root.dim_in_pixel();

                // Sliding x-axis window: last 10 seconds
                let window_duration = 10.0;
                let history_duration = history.back().map(|(t, _)| *t).unwrap_or(0.0);

                let (x_min, x_max) = if history_duration < window_duration {
                    (0.0, window_duration)
                } else {
                    (history_duration - window_duration, history_duration)
                };

                let center_midi = if current_freq <= 0.0 {
                    *last_center_midi_handle // 기존 값 사용
                } else {
                    let new_midi = midi_from_freq(current_freq);
                    last_center_midi_handle.set(new_midi); // 새 값 저장
                    new_midi
                };
                let midi_range = 5;

                // checked_sub()을 사용하여 underflow 방지
                let min_midi = center_midi
                    .checked_sub(midi_range)
                    .unwrap_or(0)
                    .clamp(0, 127);

                // checked_add()를 사용하여 overflow 방지
                let max_midi = center_midi
                    .checked_add(midi_range)
                    .unwrap_or(127)
                    .clamp(0, 127);

                // Chart 만들기: y축은 MIDI 값 그대로 사용
                let mut chart = ChartBuilder::on(&root)
                    .margin(10)
                    .set_label_area_size(LabelAreaPosition::Left, 50)
                    .set_label_area_size(LabelAreaPosition::Bottom, 40)
                    .build_cartesian_2d(x_min..x_max, min_midi as f64..max_midi as f64)
                    .unwrap();

                // Y축 라벨 설정
                chart
                    .configure_mesh()
                    .y_labels((max_midi - min_midi + 1) as usize)
                    .y_max_light_lines(0)
                    .x_max_light_lines(0)
                    .y_label_formatter(&|midi_f| note_name_from_midi(*midi_f as i32))
                    .x_desc("Time (s)")
                    .y_desc("Pitch")
                    .draw()
                    .unwrap();

                // 여러 LineSeries를 연결되지 않도록 그리기
                let mut segment: Vec<(f64, f64)> = vec![];

                for (t, freq) in history.iter() {
                    let midi = midi_float_from_freq(*freq);

                    if *freq == 0.0
                        || *t < x_min
                        || *t > x_max
                        || midi < min_midi as f64
                        || midi > max_midi as f64
                    {
                        if segment.len() > 1 {
                            chart
                                .draw_series(LineSeries::new(segment.clone(), &RED))
                                .unwrap();
                        }
                        segment.clear(); // 선을 끊음
                    } else {
                        segment.push((*t, midi));
                    }
                }

                // 마지막 세그먼트 그리기
                if segment.len() > 1 {
                    chart.draw_series(LineSeries::new(segment, &RED)).unwrap();
                }
            }

            || ()
        });
    }

    html! {
        <canvas ref={canvas_ref} width=800 height=400 />
    }
}

// MIDI 관련 함수
fn midi_from_freq(freq: f64) -> i32 {
    (12.0 * (freq / 440.0).log2() + 69.0).round() as i32
}

fn midi_float_from_freq(freq: f64) -> f64 {
    12.0 * (freq / 440.0).log2() + 69.0
}

fn freq_from_midi(midi: i32) -> f64 {
    440.0 * 2f64.powf((midi as f64 - 69.0) / 12.0)
}

fn note_name_from_midi(midi: i32) -> String {
    let notes = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let note = notes[((midi % 12 + 12) % 12) as usize];
    let octave = midi / 12 - 1;
    format!("{}{}", note, octave)
}
