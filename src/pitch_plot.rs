use log::info;
use plotters::prelude::*;
use plotters_canvas::CanvasBackend;
use std::collections::VecDeque;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, MouseEvent};
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

    // 드래그 관련 상태 추가
    let is_dragging = use_state(|| false);
    let drag_start_x = use_state(|| 0);
    let drag_start_y = use_state(|| 0);
    let view_offset_x = use_state(|| 0.0); // 시간축 오프셋 (초)
    let view_offset_y = use_state(|| 0.0); // MIDI 노트 오프셋
    let auto_follow = use_state(|| true); // 자동 따라가기 모드 (기본값: 활성화)
    let fixed_time_point = use_state(|| None::<f64>); // 고정된 시간점 (None: 자동 모드)

    // 마우스 이벤트 핸들러
    let on_mouse_down = {
        let is_dragging = is_dragging.clone();
        let drag_start_x = drag_start_x.clone();
        let drag_start_y = drag_start_y.clone();
        let auto_follow = auto_follow.clone();
        let fixed_time_point = fixed_time_point.clone();
        let history = props.history.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            is_dragging.set(true);
            drag_start_x.set(e.client_x());
            drag_start_y.set(e.client_y());
            auto_follow.set(false); // 드래그 시작시 자동 따라가기 비활성화

            // 드래그 시작할 때 현재 보고있는 시간대 고정
            if fixed_time_point.is_none() {
                let history_duration = history.back().map(|(t, _)| *t).unwrap_or(0.0);
                fixed_time_point.set(Some(history_duration));
            }
        })
    };

    let on_mouse_move = {
        let is_dragging = is_dragging.clone();
        let drag_start_x = drag_start_x.clone();
        let drag_start_y = drag_start_y.clone();
        let view_offset_x = view_offset_x.clone();
        let view_offset_y = view_offset_y.clone();
        let canvas_ref = canvas_ref.clone();
        let history = props.history.clone();

        Callback::from(move |e: MouseEvent| {
            if !*is_dragging {
                return;
            }

            if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                let canvas_width = canvas.width() as i32;
                let canvas_height = canvas.height() as i32;

                // X축 이동 (시간)
                let dx = e.client_x() - *drag_start_x;
                let window_duration = 10.0;
                let time_per_pixel = window_duration / canvas_width as f64;
                let dt = -dx as f64 * time_per_pixel;

                // 새로 계산된 오프셋값
                let new_offset_x = *view_offset_x + dt;

                // 최대/최소 제한 계산
                let history_duration = history.back().map(|(t, _)| *t).unwrap_or(0.0);
                let min_offset = -(history_duration - window_duration).max(0.0);
                let max_offset = 0.0;
                view_offset_x.set(((*view_offset_x) + dt).clamp(min_offset, max_offset));

                // Y축 이동 (MIDI 노트)
                let dy = e.client_y() - *drag_start_y;
                let midi_range = 10.0; // 화면에 표시되는 MIDI 노트 범위
                let midi_per_pixel = midi_range / canvas_height as f64;
                let dmidi = -dy as f64 * midi_per_pixel;

                // 새로운 Y 오프셋 (음높이)
                let new_offset_y = *view_offset_y + dmidi;

                // MIDI 범위: 0-127 내에서만 이동 가능하도록 제한
                let midi_range_half = 5.0;
                let min_offset_y = -69.0 + midi_range_half; // C-1(0)에 도달하는 제한
                let max_offset_y = 58.0 - midi_range_half; // G9(127)에 도달하는 제한

                view_offset_y.set(new_offset_y.max(min_offset_y).min(max_offset_y));

                // 드래그 시작점 업데이트
                drag_start_x.set(e.client_x());
                drag_start_y.set(e.client_y());
            }
        })
    };

    let on_mouse_up = {
        let is_dragging = is_dragging.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            is_dragging.set(false);
        })
    };

    let on_double_click = {
        let view_offset_x = view_offset_x.clone();
        let view_offset_y = view_offset_y.clone();
        let auto_follow = auto_follow.clone();
        let fixed_time_point = fixed_time_point.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            // 더블 클릭 시 원래 위치로 리셋
            view_offset_x.set(0.0);
            view_offset_y.set(0.0);
            auto_follow.set(true); // 자동 따라가기 다시 활성화
            fixed_time_point.set(None); // 고정 시간점 해제
        })
    };

    {
        let canvas_ref = canvas_ref.clone();
        let history = props.history.clone();
        let current_freq = props.current_freq;
        let last_center_midi_handle = last_center_midi.clone();
        let view_offset_x = view_offset_x.clone();
        let view_offset_y = view_offset_y.clone();
        let auto_follow = auto_follow.clone();
        let fixed_time_point = fixed_time_point.clone();
        info!("current_freq: {:?}", current_freq);

        use_effect_with(
            (
                history.clone(),
                current_freq,
                *view_offset_x,
                *view_offset_y,
                *auto_follow,
            ),
            move |_| {
                if let Some(canvas) = canvas_ref.cast::<web_sys::HtmlCanvasElement>() {
                    let backend = CanvasBackend::with_canvas_object(canvas).unwrap();
                    let root = backend.into_drawing_area();
                    root.fill(&WHITE).unwrap();

                    let (width, height) = root.dim_in_pixel();

                    // Sliding x-axis window: last 10 seconds
                    let window_duration = 10.0;
                    let history_duration = history.back().map(|(t, _)| *t).unwrap_or(0.0);

                    // 오프셋을 적용한 x축 범위 계산
                    let (x_min, x_max) = if *auto_follow {
                        if history_duration < window_duration {
                            (0.0, window_duration)
                        } else {
                            (history_duration - window_duration, history_duration)
                        }
                    } else {
                        let fixed_point = fixed_time_point.as_ref().unwrap_or(&history_duration);
                        let min_x = (fixed_point + *view_offset_x - window_duration).max(0.0);
                        let max_x = min_x + window_duration;
                        (min_x, max_x)
                    };

                    // 현재 중심 MIDI 노트 계산
                    let center_midi = if current_freq <= 0.0 {
                        *last_center_midi_handle // 기존 값 사용
                    } else {
                        let new_midi = midi_from_freq(current_freq);
                        if *auto_follow {
                            last_center_midi_handle.set(new_midi); // 새 값 저장 (자동 모드일 때만)
                        }
                        // new_midi
                        *last_center_midi_handle
                    };

                    // Y축 오프셋 적용
                    let adjusted_center_midi = if *auto_follow {
                        center_midi as f64
                    } else {
                        center_midi as f64 - *view_offset_y
                    };

                    let midi_range = 5;

                    // Y축 범위 계산
                    let min_midi = (adjusted_center_midi - midi_range as f64)
                        .max(0.0)
                        .min(127.0) as i32;

                    let max_midi = (adjusted_center_midi + midi_range as f64)
                        .max(0.0)
                        .min(127.0) as i32;

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

                    // 현재 모드 표시 (드래그 모드 또는 자동 모드)
                    if !*auto_follow {
                        let style = TextStyle::from(("sans-serif", 15).into_font()).color(&BLUE);
                        chart
                            .draw_series(std::iter::once(Text::new(
                                "Drag Mode (Double-click to reset)",
                                (x_min + 0.5, max_midi as f64 - 0.5),
                                &style,
                            )))
                            .unwrap();
                    }
                }

                || ()
            },
        );
    }

    html! {
        <canvas
            ref={canvas_ref}
            width=800
            height=400
            onmousedown={on_mouse_down}
            onmousemove={on_mouse_move}
            onmouseup={&on_mouse_up}
            onmouseleave={on_mouse_up.clone()}
            ondblclick={on_double_click}
            style="cursor: move;"
        />
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
