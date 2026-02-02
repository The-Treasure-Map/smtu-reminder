mod callbacks;
mod commands;

use std::sync::Arc;

use teloxide::{dispatching::UpdateFilterExt, prelude::*};

use crate::state::AppState;

pub async fn start(bot: Bot, state: Arc<AppState>) {
    let handler = dptree::entry()
        .branch(
            Update::filter_message()
                .filter_command::<commands::Command>()
                .endpoint(commands::command_endpoint),
        )
        .branch(Update::filter_callback_query().endpoint(callbacks::callback_endpoint));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .build()
        .dispatch()
        .await;
}
