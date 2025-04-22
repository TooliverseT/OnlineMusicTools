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

// 네비게이션 바 컴포넌트
#[function_component(Navbar)]
pub fn navbar() -> Html {
    html! {
        <nav class="navbar">
            <div class="navbar-container">
                <Link<Route> classes={classes!("navbar-title")} to={Route::Home}>
                    {"MusicalMind"}
                </Link<Route>>
            </div>
        </nav>
    }
}

// 메인 레이아웃 컴포넌트
#[function_component(MainLayout)]
pub fn main_layout() -> Html {
    html! {
        <>
            <Navbar />
            <div class="app-container">
                <Switch<Route> render={switch} />
            </div>
        </>
    }
}

// 상세 페이지 컴포넌트 - 피치 컨트롤
#[function_component(PitchControlsDetail)]
pub fn pitch_controls_detail() -> Html {
    html! {
        <div class="detail-page">
            <Link<Route> to={Route::Home}>{"🏠 Back to Home"}</Link<Route>>
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
            <Link<Route> to={Route::Home}>{"🏠 Back to Home"}</Link<Route>>
            <div class="content">
                <p>{"Detailed pitch analysis data and visualization."}</p>
            </div>
        </div>
    }
}

#[function_component(NotFound)]
pub fn not_found() -> Html {
    html! {
        <div>
            <Link<Route> to={Route::Home}>{"🏠 Back to Home"}</Link<Route>>
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
