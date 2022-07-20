use std::str::FromStr;
use std::sync::{Arc, Mutex};

use tauri::{api::dialog::blocking, command, CustomMenuItem, Manager, PhysicalSize, State, SystemTray, SystemTrayMenu, SystemTrayMenuItem, Window};
use tracing::{debug, error, info};

use crate::{
    config::{AgentArguments, AgentConfig, UiConfiguration, DEFAULT_AGENT_CONFIGURATION_FILE},
    server::AgentServerHandler,
};

use super::common::UiState;

const EVENT_AGENT_SERVER_START: &str = "agent-server-start-backend-event";
const EVENT_AGENT_SERVER_STOP: &str = "agent-server-stop-backend-event";
const EVENT_AGENT_EXIT: &str = "agent-exit";
const MAIN_WINDOW_LABEL: &str = "main";

#[command]
fn retrive_agent_configuration(window_state: State<'_, Arc<Mutex<UiState>>>) -> Result<AgentConfig, String> {
    let window_state = window_state.lock().map_err(|e| e.to_string())?;
    Ok(window_state.configuration.clone())
}

#[command]
fn start_agent_server(window: Window, window_state: State<'_, Arc<Mutex<UiState>>>) -> Result<(), String> {
    let window_state = window_state.lock().map_err(|e| e.to_string())?;
    let current_configuration = window_state.configuration.clone();
    println!("The agent configuraiton going to start: {current_configuration:#?}");
    window_state.agent_server_handler.start(current_configuration).map_err(|e| e.to_string())?;
    window.emit_all(EVENT_AGENT_SERVER_START, true).map_err(|e| e.to_string())?;
    Ok(())
}

#[command]
fn stop_agent_server(window: Window, window_state: State<'_, Arc<Mutex<UiState>>>) -> Result<(), String> {
    debug!("Click to stop agent server button");
    let window_state = window_state.lock().map_err(|e| e.to_string())?;
    window_state.agent_server_handler.stop().map_err(|e| e.to_string())?;
    window.emit_all(EVENT_AGENT_SERVER_STOP, true).map_err(|e| e.to_string())?;
    Ok(())
}

#[command]
fn save_agent_server_config(configuration: UiConfiguration, window_state: State<'_, Arc<Mutex<UiState>>>) -> Result<(), String> {
    println!("The configuration from ui: {configuration:#?}");
    let mut window_state = window_state.lock().map_err(|e| e.to_string())?;
    if let Some(user_token) = configuration.user_token {
        window_state.configuration.set_user_token(user_token);
    }
    if let Some(proxy_addresses) = configuration.proxy_addresses {
        window_state.configuration.set_proxy_addresses(proxy_addresses);
    }
    if let Some(port) = configuration.port {
        window_state.configuration.set_port(u16::from_str(port.as_str()).map_err(|e| e.to_string())?);
    }
    let configuration_to_save = window_state.configuration.clone();
    let configuration_file_content = toml::to_string_pretty(&configuration_to_save).map_err(|e| e.to_string())?;
    let configuration_file_path = window_state.arguments.configuration_file.as_deref().unwrap_or(DEFAULT_AGENT_CONFIGURATION_FILE);
    std::fs::write(configuration_file_path, configuration_file_content).map_err(|e| e.to_string())?;
    println!("The configuration saved: {:#?}", window_state.configuration);
    Ok(())
}

pub(crate) struct MainFrame {
    system_tray: SystemTray,
    agent_server_handler: AgentServerHandler,
    configuration: AgentConfig,
    arguments: AgentArguments,
}
impl MainFrame {
    pub fn new(agent_server_handler: AgentServerHandler, configuration: AgentConfig, arguments: AgentArguments) -> Self {
        let exit_system_tray_menu_item = CustomMenuItem::new(EVENT_AGENT_EXIT.to_string(), "Exit");
        let start_system_tray_menu_item = CustomMenuItem::new(EVENT_AGENT_SERVER_START.to_string(), "Start");
        let stop_system_tray_menu_item = CustomMenuItem::new(EVENT_AGENT_SERVER_STOP.to_string(), "Stop");
        let system_tray_menu = SystemTrayMenu::new()
            .add_item(start_system_tray_menu_item)
            .add_item(stop_system_tray_menu_item)
            .add_native_item(SystemTrayMenuItem::Separator)
            .add_item(exit_system_tray_menu_item);
        let system_tray = SystemTray::new().with_menu(system_tray_menu);
        Self {
            system_tray,
            agent_server_handler,
            configuration,
            arguments,
        }
    }

    pub(crate) fn run(self) {
        tauri::Builder::default()
            .invoke_handler(tauri::generate_handler![
                start_agent_server,
                stop_agent_server,
                save_agent_server_config,
                retrive_agent_configuration
            ])
            .system_tray(self.system_tray)
            .manage(Arc::new(Mutex::new(UiState {
                agent_server_handler: self.agent_server_handler,
                configuration: self.configuration,
                arguments: self.arguments,
            })))
            .on_window_event(|event| match event.event() {
                tauri::WindowEvent::CloseRequested { .. } => {
                    info!("Close agent GUI window.");
                    std::process::exit(0);
                },
                tauri::WindowEvent::Resized(PhysicalSize { width, height }) => {
                    if *width == 0 && *height == 0 {
                        debug!("Going to hide agent GUI on minimize window.");
                        if let Err(e) = event.window().hide() {
                            error!("Fail to hide agent window because of error: {e:#?}");
                        };
                    }
                },
                event => {
                    debug!("Ignore other window event: {event:#?}");
                },
            })
            .on_system_tray_event(|app, event| {
                let main_window = app.get_window(MAIN_WINDOW_LABEL).unwrap();
                match event {
                    tauri::SystemTrayEvent::LeftClick { .. } => {
                        if let Ok(true) = main_window.is_visible() {
                            if let Err(e) = main_window.hide() {
                                error!("Fail to hide agent window because of error: {e:#?}");
                            };
                        } else {
                            if let Err(e) = main_window.show() {
                                error!("Fail to show agent window because of error: {e:#?}");
                            };
                        };
                    },
                    tauri::SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
                        EVENT_AGENT_EXIT => {
                            info!("Close agent GUI window.");
                            std::process::exit(0);
                        },
                        EVENT_AGENT_SERVER_START => {
                            let window_state = main_window.state::<Arc<Mutex<UiState>>>().clone();
                            if let Ok(window_state) = window_state.lock() {
                                let current_configuration = window_state.configuration.clone();
                                if let Err(e) = window_state.agent_server_handler.start(current_configuration) {
                                    blocking::message(Some(&main_window), "Error", e.to_string());
                                    error!("Fail to start agent server because of exception: {e:#?}");
                                };
                                if let Err(e) = main_window.emit_all(EVENT_AGENT_SERVER_START, true) {
                                    error!("Fail to send start single to agent server ui because of error: {e:#?}");
                                };
                            };
                        },
                        EVENT_AGENT_SERVER_STOP => {
                            let window_state = main_window.state::<Arc<Mutex<UiState>>>().clone();
                            if let Ok(window_state) = window_state.lock() {
                                if let Err(e) = window_state.agent_server_handler.stop() {
                                    blocking::message(Some(&main_window), "Error", e.to_string());
                                    error!("Fail to stop agent server because of exception: {e:#?}");
                                };
                                if let Err(e) = main_window.emit_all(EVENT_AGENT_SERVER_STOP, true) {
                                    error!("Fail to send stop single to agent server ui because of error: {e:#?}");
                                };
                            };
                        },
                        unknown => {
                            debug!("Nothing to do because of unknown system tray menu item: {}", unknown)
                        },
                    },
                    _ => {
                        debug!("System tray event happen.");
                    },
                }
            })
            .run(tauri::generate_context!())
            .expect("Error happen while create agent GUI.");
    }
}
