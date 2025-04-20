use js_sys::Date;
use log::info;
use plotters::prelude::*;
use plotters_canvas::CanvasBackend;
use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::f64::consts::LOG10_E;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, MouseEvent};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct PitchPlotProps {
    pub current_freq: f64,
    pub history: VecDeque<(f64, Vec<(f64, f32)>)>, // (timestamp, [(frequency, amplitude)])
}

#[function_component(PitchPlot)]
pub fn pitch_plot(props: &PitchPlotProps) -> Html {
    let canvas_ref = use_node_ref();
    let last_center_midi = use_state(|| 69); // MIDI 69 (A4)를 기본값으로 설정
    let last_center_freq = use_state(|| 440.0); // A4 주파수를 기본값으로 설정

    // 애니메이션을 위한 상태 추가
    let target_center_freq = use_state(|| 440.0); // 목표 중심 주파수
    let transition_start_time = use_state(|| 0.0); // 전환 시작 시간
    let transition_duration = use_state(|| 0.5); // 전환 지속 시간 (초)
    let is_transitioning = use_state(|| false); // 전환 중인지 여부

    // 드래그 관련 상태 추가
    let is_dragging = use_state(|| false);
    let drag_start_x = use_state(|| 0);
    let drag_start_y = use_state(|| 0);
    let view_offset_x = use_state(|| 0.0); // 시간축 오프셋 (초)
    let freq_ratio = use_state(|| 1.0); // 주파수 비율 오프셋 (곱하는 값, 1.0이 기본값)
    let auto_follow = use_state(|| true); // 자동 따라가기 모드 (기본값: 활성화)

    // 고정 시간 범위를 위한 상태 추가
    let fixed_time_range = use_state(|| None::<(f64, f64)>); // 고정된 시간 범위 (시작, 끝)

    // 마우스 이벤트 핸들러
    let on_mouse_down = {
        let is_dragging = is_dragging.clone();
        let drag_start_x = drag_start_x.clone();
        let drag_start_y = drag_start_y.clone();
        let auto_follow = auto_follow.clone();
        let fixed_time_range = fixed_time_range.clone();
        let history = props.history.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            is_dragging.set(true);
            drag_start_x.set(e.client_x());
            drag_start_y.set(e.client_y());

            // 드래그 시작시 자동 따라가기 비활성화
            if *auto_follow {
                auto_follow.set(false);

                // 현재 보고 있는 시간 범위를 고정
                let window_duration = 30.0;
                let history_duration = history.back().map(|(t, _)| *t).unwrap_or(0.0);

                let x_min = if history_duration < window_duration {
                    0.0
                } else {
                    history_duration - window_duration
                };

                let x_max = x_min + window_duration;
                fixed_time_range.set(Some((x_min, x_max)));
            }
        })
    };

    let on_mouse_move = {
        let is_dragging = is_dragging.clone();
        let drag_start_x = drag_start_x.clone();
        let drag_start_y = drag_start_y.clone();
        let view_offset_x = view_offset_x.clone();
        let freq_ratio = freq_ratio.clone();
        let canvas_ref = canvas_ref.clone();
        let history = props.history.clone();
        let fixed_time_range = fixed_time_range.clone();
        let last_center_freq = last_center_freq.clone();

        Callback::from(move |e: MouseEvent| {
            if !*is_dragging {
                return;
            }

            if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                let canvas_width = canvas.width() as i32;
                let canvas_height = canvas.height() as i32;

                // X축 이동 (시간)
                let dx = e.client_x() - *drag_start_x;
                let window_duration = 30.0;
                let time_per_pixel = window_duration / canvas_width as f64;
                let dt = -dx as f64 * time_per_pixel;

                // 현재 고정된 시간 범위가 있다면 그것을 기준으로 이동
                if let Some((current_min, current_max)) = *fixed_time_range {
                    let new_min = current_min + dt;
                    let new_max = current_max + dt;

                    // 최대 히스토리 길이를 넘어서지 않도록 제한
                    let history_duration = history.back().map(|(t, _)| *t).unwrap_or(0.0);
                    if new_max <= history_duration && new_min >= 0.0 {
                        fixed_time_range.set(Some((new_min, new_max)));
                    }
                }

                // Y축 이동 (주파수 스케일) - 주파수 비율로 계산
                let dy = e.client_y() - *drag_start_y;
                let freq_range_factor = 1.5f64; // 화면에 표시되는 주파수 범위 비율
                let freq_range_log = freq_range_factor.ln() * 2.0; // 로그 스케일에서의 범위
                let log_per_pixel = freq_range_log / canvas_height as f64;

                // 마우스 이동에 따른 주파수 비율 변화량 계산 (로그 스케일 기반)
                let dfreq_ratio = (-dy as f64 * log_per_pixel).exp();

                // 새 주파수 비율 적용 (곱셈으로 비율 변화 적용)
                let new_freq_ratio = *freq_ratio * dfreq_ratio;

                // 주파수 비율 범위 제한 (너무 높거나 낮은 주파수로 이동하지 않도록)
                // MIDI 0 (C-1)에서 127 (G9)까지의 주파수 범위 내에서 제한
                let min_freq = freq_from_midi(0); // C-1
                let max_freq = freq_from_midi(127); // G9
                let base_freq = *last_center_freq;

                let min_ratio = min_freq / base_freq * 2.0; // 최소 주파수 비율 (약간의 여유 추가)
                let max_ratio = max_freq / base_freq * 0.5; // 최대 주파수 비율 (약간의 여유 추가)

                // 클램핑하여 비율을 제한하고 상태 업데이트
                if new_freq_ratio < min_ratio {
                    freq_ratio.set(min_ratio);
                } else if new_freq_ratio > max_ratio {
                    freq_ratio.set(max_ratio);
                } else {
                    freq_ratio.set(new_freq_ratio);
                }

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
        let freq_ratio = freq_ratio.clone();
        let auto_follow = auto_follow.clone();
        let fixed_time_range = fixed_time_range.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            // 더블 클릭 시 원래 위치로 리셋
            view_offset_x.set(0.0);
            freq_ratio.set(1.0); // 주파수 비율 리셋 (1.0 = 원래 비율)
            auto_follow.set(true); // 자동 따라가기 다시 활성화
            fixed_time_range.set(None); // 고정 시간 범위 해제
        })
    };

    // 부드러운 전환을 위한 함수
    fn ease_out_cubic(x: f64) -> f64 {
        1.0 - (1.0 - x).powi(3)
    }

    {
        let canvas_ref = canvas_ref.clone();
        let history = props.history.clone();
        let current_freq = props.current_freq;
        let last_center_midi_handle = last_center_midi.clone();
        let last_center_freq_handle = last_center_freq.clone();
        let freq_ratio = freq_ratio.clone();
        let auto_follow = auto_follow.clone();
        let fixed_time_range = fixed_time_range.clone();
        let target_center_freq = target_center_freq.clone();
        let transition_start_time = transition_start_time.clone();
        let transition_duration = transition_duration.clone();
        let is_transitioning = is_transitioning.clone();

        use_effect_with(
            (
                history.clone(),
                current_freq,
                *freq_ratio,
                *auto_follow,
                fixed_time_range.clone(),
                *is_transitioning,
            ),
            move |_| {
                // 현재 시간 얻기 (초 단위)
                let current_time = Date::now() / 1000.0;

                if let Some(canvas) = canvas_ref.cast::<web_sys::HtmlCanvasElement>() {
                    // 주파수가 변경되었고, 자동 따라가기 모드일 때 처리
                    if *auto_follow && current_freq > 0.0 {
                        // 현재 표시 중인 주파수와 새 주파수의 차이가 큰 경우 부드러운 전환
                        let current_center = *last_center_freq_handle;
                        let new_freq = current_freq;

                        // 현재 주파수와 새 주파수의 MIDI 노트 값 차이로 범위 밖 여부 확인
                        let current_midi = midi_float_from_freq(current_center);
                        let new_midi = midi_float_from_freq(new_freq);
                        let midi_diff = (new_midi - current_midi).abs();

                        // MIDI 노트 값 차이가 충분히 큰 경우(≈반음 이상) 전환 시작
                        if midi_diff > 1.0 && !*is_transitioning {
                            // 새로운 전환 시작
                            target_center_freq.set(new_freq);
                            transition_start_time.set(current_time);
                            is_transitioning.set(true);
                        } else if !*is_transitioning {
                            // 작은 변화는 즉시 적용
                            last_center_midi_handle.set(midi_from_freq(new_freq));
                            last_center_freq_handle.set(new_freq);
                        }
                    }

                    // 전환 중이라면 진행 상태 계산
                    if *is_transitioning {
                        let elapsed = current_time - *transition_start_time;
                        let progress = (elapsed / *transition_duration).min(1.0);

                        if progress >= 1.0 {
                            // 전환 완료
                            is_transitioning.set(false);
                            last_center_freq_handle.set(*target_center_freq);
                            last_center_midi_handle.set(midi_from_freq(*target_center_freq));
                        } else {
                            // 전환 진행 중 - 중간값 계산
                            let t = ease_out_cubic(progress);
                            let start_freq = *last_center_freq_handle;
                            let target_freq = *target_center_freq;

                            // 로그 스케일로 보간
                            let log_start = start_freq.ln();
                            let log_target = target_freq.ln();
                            let log_current = log_start + (log_target - log_start) * t;
                            let current_freq = log_current.exp();

                            // 현재 중간값 적용
                            last_center_freq_handle.set(current_freq);
                            last_center_midi_handle.set(midi_from_freq(current_freq));
                        }
                    }

                    let backend = CanvasBackend::with_canvas_object(canvas).unwrap();
                    let root = backend.into_drawing_area();
                    root.fill(&WHITE).unwrap();

                    let (_width, height) = root.dim_in_pixel();

                    // 시간 범위 계산
                    let window_duration = 30.0;
                    let history_duration = history.back().map(|(t, _)| *t).unwrap_or(0.0);

                    // 고정 모드 또는 자동 모드에 따라 x축 범위 계산
                    let (x_min, x_max) = if let Some((min, max)) = *fixed_time_range {
                        // 고정 모드: 사용자가 드래그한 범위 사용
                        (min, max)
                    } else if *auto_follow {
                        // 자동 모드: 최신 데이터 표시
                        if history_duration < window_duration {
                            (0.0, window_duration)
                        } else {
                            (history_duration - window_duration, history_duration)
                        }
                    } else {
                        // 예전 방식 (기존 코드 호환성)
                        if history_duration < window_duration {
                            (0.0, window_duration)
                        } else {
                            (history_duration - window_duration, history_duration)
                        }
                    };

                    // 현재 중심 주파수 계산 (전환 중이면 보간된 값 사용)
                    let center_freq = if current_freq <= 0.0 {
                        // 주파수가 0이면 마지막 저장된 주파수 사용
                        *last_center_freq_handle
                    } else {
                        // 전환 중이거나 자동 모드일 때는 이미 last_center_freq_handle 업데이트됨
                        *last_center_freq_handle
                    };

                    // Y축 오프셋 적용 (주파수 비율 단위)
                    let adjusted_center_freq = if *auto_follow {
                        center_freq
                    } else {
                        // freq_ratio는 주파수 비율이므로 곱하기로 적용
                        center_freq * *freq_ratio
                    };

                    // 주파수 범위 계산 (옥타브 단위로 설정)
                    let freq_range_factor = 1.5; // 중심 주파수의 몇 배까지 표시할지 (1.5 = ±반옥타브)

                    let min_freq = adjusted_center_freq / freq_range_factor;
                    let max_freq = adjusted_center_freq * freq_range_factor;

                    // 참조용: 해당 주파수 범위에 해당하는 MIDI 노트 범위 계산
                    let min_midi = midi_from_freq(min_freq);
                    let max_midi = midi_from_freq(max_freq);

                    let min_log = min_freq.log10();
                    let max_log = max_freq.log10();

                    // Chart 만들기: y축은 주파수(Hz) 값을 사용
                    let mut chart = ChartBuilder::on(&root)
                        .margin(10)
                        .set_label_area_size(LabelAreaPosition::Left, 50)
                        .set_label_area_size(LabelAreaPosition::Bottom, 30)
                        .build_cartesian_2d(x_min..x_max, min_log..max_log) // 로그 스케일 범위 사용
                        .unwrap();

                    // 라벨과 보조선 위치 설정
                    let mut y_labels: Vec<(f64, String, bool)> = Vec::new();
                    let mut grid_lines: Vec<f64> = Vec::new();

                    // 현재 주파수에 가장 가까운 MIDI 노트 계산
                    let closest_midi = midi_from_freq(current_freq);
                    let closest_freq = freq_from_midi(closest_midi);
                    let closest_log_freq = closest_freq.log10();

                    // MIDI 노트에 해당하는 주파수에만 라벨과 보조선 표시
                    for midi in min_midi..=max_midi {
                        if midi != min_midi && midi != max_midi {
                            let freq = freq_from_midi(midi);
                            let log_freq = freq.log10();
                            let name = note_name_from_midi(midi);
                            y_labels.push((log_freq, name, midi == closest_midi));
                            grid_lines.push(log_freq);
                        }
                    }

                    // 메쉬 설정 (y 라벨은 비활성화)
                    chart
                        .configure_mesh()
                        .x_desc("Time (s)")
                        .y_desc("Musical Note")
                        .x_labels(5)
                        .y_labels(0)
                        .y_label_formatter(&|_| String::new())
                        .draw()
                        .unwrap();

                    // 직접 y축 라벨과 가로선 그리기
                    for (log_freq, label, is_closest) in y_labels.iter() {
                        // 가로선 추가 - 가장 가까운 노트는 다른 색상으로 표시
                        let line_color = if *is_closest {
                            // 현재 주파수에 가장 가까운 노트는 파란색 라인
                            RGBColor(100, 149, 237) // 콘플라워 블루
                        } else {
                            // 나머지는 기존 회색 라인
                            RGBColor(200, 200, 200)
                        };

                        let line_width = if *is_closest { 2 } else { 1 };

                        chart
                            .draw_series(std::iter::once(PathElement::new(
                                vec![(x_min, *log_freq), (x_max, *log_freq)],
                                ShapeStyle::from(&line_color).stroke_width(line_width),
                            )))
                            .unwrap();

                        // Y축 라벨을 차트 왼쪽 영역에 그리기
                        // 좌표 변환 직접 계산: 차트 왼쪽 영역에 라벨 배치
                        let font_weight = if *is_closest { "bold" } else { "normal" };
                        let font_size = if *is_closest { 17.0 } else { 15.0 };
                        let font_desc = format!("{}px {} sans-serif", font_size, font_weight);
                        let style = TextStyle::from(font_desc.into_font());

                        // 가장 가까운 노트는 텍스트 색상도 변경
                        let text_color = if *is_closest {
                            &RGBColor(65, 105, 225) // 로열 블루
                        } else {
                            &BLACK
                        };

                        // 로그 주파수 값을 정규화하여 Y 좌표로 변환 (0.0 ~ 1.0 범위로)
                        let normalized_y = (max_log - *log_freq) / (max_log - min_log);

                        // 차트 영역 상단 및 하단 여백 계산 (차트 영역 기준)
                        let chart_top_margin = 10i32; // 차트 상단 여백 (명시적으로 i32로 지정)
                        let chart_bottom_margin = 40i32; // 차트 하단 여백 (x축 라벨 포함)

                        // 차트 내부 영역 높이
                        let chart_inner_height =
                            height as i32 - chart_top_margin - chart_bottom_margin;

                        // 정규화된 값을 픽셀 Y 좌표로 변환 (차트 영역 내에서)
                        let pixel_y =
                            (normalized_y * chart_inner_height as f64) as i32 + chart_top_margin;

                        // 텍스트가 정확히 가로선 중앙에 위치하도록 조정
                        // 폰트 크기의 절반을 기본값으로 설정하고, 위치에 따라 점진적으로 조정
                        // 위치에 따른 보정 계수 계산 (위쪽은 작게, 아래쪽은 크게)
                        // normalized_y는 0.0(위)에서 1.0(아래)의 값을 가짐
                        let position_factor = 0.5 + normalized_y * 0.0; // 0.7에서 1.4까지 변화

                        let text_vertical_center_offset =
                            (font_size * position_factor / 2.0) as i32;

                        // 차트 왼쪽 영역에 텍스트 그리기
                        root.draw_text(
                            &label,
                            &(style.color(text_color)),
                            (30, pixel_y - text_vertical_center_offset), // 수직 및 수평 위치 조정
                        )
                        .unwrap();
                    }

                    // 모든 시간대에 대해 점 그리기 및 각 시간대의 최대 진폭 찾기
                    let mut time_grouped_points: BTreeMap<i64, Vec<(f64, f32)>> = BTreeMap::new();

                    // 시간별로 데이터 그룹화
                    for (t, freqs) in history.iter() {
                        if *t < x_min || *t > x_max {
                            // 시간 범위 밖이면 스킵
                            continue;
                        }

                        let mut valid_freqs = Vec::new();
                        for (freq, amplitude) in freqs {
                            if *freq == 0.0 {
                                continue;
                            }

                            let log_freq = freq.log10();
                            // 범위 내 주파수만 저장
                            if log_freq >= min_log && log_freq <= max_log {
                                valid_freqs.push((*freq, *amplitude));
                            }
                        }

                        // 유효한 주파수가 있으면 저장
                        if !valid_freqs.is_empty() {
                            // 시간 값을 정수로 변환 (밀리초 단위)
                            let time_key = (*t * 1000.0) as i64;
                            time_grouped_points.insert(time_key, valid_freqs);
                        }
                    }

                    // 현재 시간에 대한 세로선 그리기
                    // 현재 시간 (history의 마지막 시간)
                    let current_time = history.back().map(|(t, _)| *t).unwrap_or(0.0);

                    // 현재 시간이 표시 범위 내에 있는 경우에만 세로선 표시
                    if current_time >= x_min && current_time <= x_max {
                        // 현재 시간 세로선 스타일 설정
                        let line_color = RGBColor(50, 180, 50); // 초록색 계열
                        let line_style = ShapeStyle::from(&line_color).stroke_width(2);

                        // 현재 시간 세로선 그리기
                        chart
                            .draw_series(std::iter::once(PathElement::new(
                                vec![(current_time, min_log), (current_time, max_log)],
                                line_style,
                            )))
                            .unwrap();

                        // // 현재 시간 라벨 표시
                        // let time_label = format!("{:.1}s", current_time);
                        // let text_style = TextStyle::from(("sans-serif", 14).into_font())
                        //     .color(&RGBColor(50, 180, 50));

                        // chart
                        //     .draw_series(std::iter::once(Text::new(
                        //         time_label,
                        //         (current_time, max_log),
                        //         &text_style,
                        //     )))
                        //     .unwrap();
                    }

                    // 가장 최근의 가장 강한 주파수만 크기 3으로, 나머지는 2로 설정
                    let latest_time_key = time_grouped_points.keys().max().cloned();

                    // 각 시간대별로 처리
                    for (time_key, freqs) in time_grouped_points.iter() {
                        // 진폭 기준 내림차순 정렬
                        let mut sorted_freqs = freqs.clone();
                        sorted_freqs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

                        // 원래 시간 값으로 변환
                        let t = *time_key as f64 / 1000.0;

                        // 각 주파수에 대해 점 그리기
                        for (i, (freq, amplitude)) in sorted_freqs.iter().enumerate() {
                            let log_freq = freq.log10();

                            // 첫 번째(가장 강한 주파수)라도 진폭이 0.7 이상인 경우에만 강조
                            let color = if i == 0 && *amplitude >= 0.7 {
                                // 가장 강한 주파수는 진한 빨간색
                                RGBColor(255, 0, 0)
                            } else {
                                // 나머지는 매우 옅은 회색계열
                                let alpha = (amplitude * 100.0) as u8 + 40; // 40~140 범위로 더 옅게 조정
                                RGBColor(200, 200, 200)
                            };

                            // 가장 최근의 가장 강한 주파수만 크기 3으로, 나머지는 2로 설정
                            let point_size = if i == 0
                                && Some(*time_key) == latest_time_key
                                && *amplitude >= 0.7
                            {
                                5 // 가장 최근의 가장 강한 주파수는 크게
                            } else {
                                2 // 나머지는 작게
                            };

                            chart
                                .draw_series(std::iter::once(Circle::new(
                                    (t, log_freq),
                                    point_size,
                                    color.filled(),
                                )))
                                .unwrap();
                        }
                    }

                    // 현재 모드 표시 (드래그 모드 또는 자동 모드)
                    if !*auto_follow {
                        let style = TextStyle::from(("sans-serif", 15).into_font()).color(&BLUE);
                        chart
                            .draw_series(std::iter::once(Text::new(
                                "Drag Mode (Double-click to reset)",
                                (x_min + 0.5, max_log - 0.05),
                                &style,
                            )))
                            .unwrap();

                        // 고정된 시간 범위 정보 표시
                        // if let Some((min, max)) = *fixed_time_range {
                        //     let time_info = format!("Time: {:.1}s - {:.1}s", min, max);
                        //     chart
                        //         .draw_series(std::iter::once(Text::new(
                        //             time_info,
                        //             (x_min + 0.5, max_log - 0.15),
                        //             &style,
                        //         )))
                        //         .unwrap();
                        // }
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
