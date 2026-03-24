pub mod config;
pub mod demos;
pub mod repl;

use repl::Repl;
use yew::prelude::*;

#[function_component(App)]
pub fn app() -> Html {
    html! {
        <Repl />
    }
}
