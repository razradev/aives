use kokoros::tts::koko::TTSKoko;
use ollama_rs::generation::chat::ChatMessage;
use ollama_rs::models::ModelOptions;
use ollama_rs::Ollama;
use std::sync::Arc;
use tauri::path::BaseDirectory::Resource;
use tauri::Manager;
use tokio::sync::Mutex;
mod ckokoros2;
mod cmouse;
mod collama;

struct AppState {
    pub ollama: Mutex<Ollama>,
    pub options: Mutex<ModelOptions>,
    pub history: Mutex<Vec<ChatMessage>>,
    pub tts_instance: Arc<Mutex<Option<TTSKoko>>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();
    tauri::Builder::default()
        .setup(|app| {
            app.manage(AppState {
                ollama: Mutex::new(Ollama::default()),
                options: Mutex::new(ModelOptions::default()),
                history: Mutex::new(vec![ChatMessage::system("System prompt".to_string())]),
                tts_instance: Arc::new(Mutex::new(None)),
            });

            let app_handle = app.handle();

            let _ = tokio::runtime::Runtime::new().unwrap().block_on(async {
                let model_path_buf = app_handle
                    .path()
                    .resolve("resources/kokoro-v1.0.onnx", Resource)
                    .expect("Failed to resolve Kokoro model path");
                let voices_path_buf = app_handle
                    .path()
                    .resolve("resources/voices-v1.0.bin", Resource)
                    .expect("Failed to resolve Kokoro voice path");

                let app_state = app_handle.state::<AppState>();

                ckokoros2::setup_tauri_commands(
                    model_path_buf.to_str().unwrap(),
                    voices_path_buf.to_str().unwrap(),
                    app_state.tts_instance.clone(),
                )
                .await
                .expect("Failed to set up Tauri commands and state");

                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            });

            Ok(())
        })
        .plugin(tauri_plugin_positioner::init())
        .invoke_handler(tauri::generate_handler![
            cmouse::check_cursor_region,
            collama::gen_res,
            ckokoros2::generate_speech,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
