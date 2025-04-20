use yew::prelude::*;
use yew_router::prelude::*;

use crate::dashboard::{Dashboard, DashboardItem, DashboardLayout};
use crate::pitch_plot::PitchPlot;
use crate::PitchAnalyzer;

// 애플리케이션의 라우트 정의
#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Home,
    #[at("/pitch-controls")]
    PitchControls,
    #[at("/pitch-plot")]
    PitchPlot,
    #[not_found]
    #[at("/404")]
    NotFound,
}

// 상세 페이지 컴포넌트 - 피치 컨트롤
#[function_component(PitchControlsDetail)]
pub fn pitch_controls_detail() -> Html {
    html! {
        <div class="detail-page">
            <h1>{"🎚️ 피치 컨트롤 상세 페이지"}</h1>
            <Link<Route> to={Route::Home}>{"🏠 홈으로 돌아가기"}</Link<Route>>
            <div class="content">
                <PitchAnalyzer />
            </div>
        </div>
    }
}

// 상세 페이지 컴포넌트 - 피치 플롯
#[function_component(PitchPlotDetail)]
pub fn pitch_plot_detail() -> Html {
    // 빈 데이터로 PitchPlot 컴포넌트 렌더링
    // 실제 구현에서는 저장된 데이터를 불러오거나 API를 통해 데이터를 가져올 수 있음
    html! {
        <div class="detail-page">
            <h1>{"📊 피치 플롯 상세 페이지"}</h1>
            <Link<Route> to={Route::Home}>{"🏠 홈으로 돌아가기"}</Link<Route>>
            <div class="content">
                <p>{"이 페이지에서는 더 자세한 피치 분석 데이터를 볼 수 있습니다."}</p>
            </div>
        </div>
    }
}

#[function_component(NotFound)]
pub fn not_found() -> Html {
    html! {
        <div>
            <h1>{"404 - 페이지를 찾을 수 없습니다"}</h1>
            <Link<Route> to={Route::Home}>{"🏠 홈으로 돌아가기"}</Link<Route>>
        </div>
    }
}

pub fn switch(routes: Route) -> Html {
    match routes {
        Route::Home => html! { <PitchAnalyzer /> },
        Route::PitchControls => html! { <PitchControlsDetail /> },
        Route::PitchPlot => html! { <PitchPlotDetail /> },
        Route::NotFound => html! { <NotFound /> },
    }
}
