use ollama_rs::generation::chat::request::ChatMessageRequest;
use ollama_rs::generation::chat::ChatMessage;
use tauri::State;

use crate::AppState;
const MODEL: &str = "hf.co/mradermacher/Celeste-12B-V1.6-GGUF:Q4_K_M";

#[tauri::command]
pub async fn gen_res(prompt: &str, state: State<'_, AppState>) -> Result<String, String> {
    let mut ollama_client_guard = state.ollama.lock().await;
    let ollama_options_guard = state.options.lock().await;
    let mut chat_history_guard = state.history.lock().await;

    let user_message = ChatMessage::user(prompt.to_string());

    let result = ollama_client_guard
        .send_chat_messages_with_history(
            &mut *chat_history_guard,
            ChatMessageRequest::new(MODEL.to_string(), vec![user_message])
                .options(ollama_options_guard.clone()),
        )
        .await
        .map_err(|e| e.to_string())?;

    let assistant_message = result.message.content;

    Ok(assistant_message.to_string())
}
