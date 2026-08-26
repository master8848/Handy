/**
 * Tauri mock injected via Playwright addInitScript before app code runs.
 * Bypasses onboarding, mocks models/settings so every persona renders
 * without the Rust backend.
 *
 * Keep mockSettings in sync with src-tauri/src/settings.rs get_default_settings().
 * Add new bindings here when they are added there.
 */
export const TAURI_MOCK_SCRIPT = `
(() => {
  const listeners = new Map();
  let nextId = 1;
  const callbacks = new Map();

  const mockSettings = {
    settings_schema_version: 2,
    bindings: {
      transcribe: { id: "transcribe", name: "Transcribe", description: "", default_binding: "option+space", current_binding: "option+space" },
      transcribe_with_post_process: { id: "transcribe_with_post_process", name: "Transcribe with Post-Processing", description: "", default_binding: "option+shift+space", current_binding: "option+shift+space" },
      cancel: { id: "cancel", name: "Cancel", description: "", default_binding: "escape", current_binding: "escape" },
      prompt_palette: { id: "prompt_palette", name: "Prompt Palette", description: "", default_binding: "option+shift+p", current_binding: "option+shift+p" },
      quick_prompt: { id: "quick_prompt", name: "Quick Prompt", description: "Open the quick prompt box (Raycast/Spotlight style) to write a snippet and paste it with Cmd+Enter.", default_binding: "command+shift+j", current_binding: "command+shift+j" }
    },
    push_to_talk: true,
    audio_feedback: false,
    audio_feedback_volume: 1,
    sound_theme: "marimba",
    start_hidden: false,
    autostart_enabled: false,
    update_checks_enabled: true,
    show_whats_new_on_update: true,
    whats_new_last_seen_version: "0.9.4",
    selected_model: "mock-model",
    onboarding_completed: true,
    always_on_microphone: false,
    selected_microphone: "Default",
    clamshell_microphone: "Default",
    selected_output_device: "Default",
    translate_to_english: false,
    selected_language: "auto",
    overlay_position: "bottom",
    overlay_style: "live",
    overlay_native_enabled: false,
    debug_mode: false,
    log_level: "info",
    custom_words: [],
    custom_word_datasets: [],
    text_replacements: [],
    model_unload_timeout: "min_5",
    multi_model_loading: false,
    word_correction_threshold: 0.18,
    history_limit: 5,
    recording_retention_period: "preserve_limit",
    paste_method: "ctrl_v",
    clipboard_handling: "dont_modify",
    auto_submit: false,
    auto_submit_key: "enter",
    post_process_enabled: false,
    post_process_provider_id: "openai",
    post_process_providers: [],
    post_process_api_keys: {},
    post_process_models: {},
    post_process_prompts: [],
    post_process_selected_prompt_id: null,
    mute_while_recording: false,
    append_trailing_space: false,
    app_language: "en",
    theme: "system",
    accent_color: "pink",
    experimental_enabled: false,
    lazy_stream_close: false,
    keyboard_implementation: "handy_keys",
    show_tray_icon: true,
    paste_delay_ms: 60,
    paste_delay_after_ms: 60,
    reliable_paste: false,
    typing_tool: "auto",
    external_script_path: null,
    custom_filler_words: null,
    transcribe_accelerator: "auto",
    ort_accelerator: "auto",
    transcribe_gpu_device: -1,
    extra_recording_buffer_ms: 0,
    vad_enabled: true,
    spell_check_enabled: false,
    prompt_library_enabled: true,
    server_mode_enabled: false,
    server_port: 17373,
    server_bind: "127.0.0.1",
    server_auth_token: null
  };

  window.__TAURI_INTERNALS__ = window.__TAURI_INTERNALS__ || {};
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = window.__TAURI_EVENT_PLUGIN_INTERNALS__ || {};
  window.__TAURI_OS_PLUGIN_INTERNALS__ = {
    platform: "macos",
    version: "14.0.0",
    family: "unix",
    arch: "aarch64",
    exe_extension: "",
    os_type: "macos",
    eol: "\\n"
  };
  window.__TAURI_STORE_PLUGIN_INTERNALS__ = window.__TAURI_STORE_PLUGIN_INTERNALS__ || {};

  function handleEventPlugin(cmd, args) {
    switch (cmd) {
      case "plugin:event|listen": {
        const event = args.event;
        const handler = args.handler;
        if (!listeners.has(event)) listeners.set(event, []);
        listeners.get(event).push(handler);
        return handler;
      }
      case "plugin:event|emit": {
        const { event, payload } = args;
        const hs = listeners.get(event) || [];
        hs.forEach((id) => {
          const cb = callbacks.get(id);
          if (cb) cb({ event, id: 0, payload });
        });
        return null;
      }
      case "plugin:event|unlisten": {
        const { event, id } = args;
        const arr = listeners.get(event);
        if (arr) {
          const idx = arr.indexOf(id);
          if (idx !== -1) arr.splice(idx, 1);
        }
        callbacks.delete(id);
        return null;
      }
      case "plugin:event|register_listener":
      case "plugin:event|remove_listener":
        return null;
      default: return null;
    }
  }

  async function mockInvoke(cmd, args) {
    if (cmd.startsWith("plugin:event|")) return handleEventPlugin(cmd, args);
    if (cmd.startsWith("plugin:os|")) {
      if (cmd === "plugin:os|locale") return "en-US";
      if (cmd === "plugin:os|hostname") return "localhost";
      return null;
    }
    if (cmd.startsWith("plugin:store|")) return null;
    if (cmd.includes("macos-permissions") || cmd.includes("macos_permissions")) return true;
    switch (cmd) {
      case "get_app_settings": return mockSettings;
      case "get_default_settings": return mockSettings;
      case "get_available_models": return [];
      case "get_model_info": return null;
      case "get_current_model": return "";
      case "get_loaded_models": return [];
      case "get_transcription_model_status": return null;
      case "is_model_loading": return false;
      case "get_available_microphones": return [];
      case "get_available_output_devices": return [];
      case "get_selected_microphone": return "Default";
      case "get_selected_output_device": return "Default";
      case "get_clamshell_microphone": return "Default";
      case "get_app_dir_path": return "/tmp";
      case "get_log_dir_path": return "/tmp";
      case "check_custom_sounds": return { start: false, stop: false };
      case "get_history_entries": return { entries: [], has_more: false };
      case "get_audio_file_path": return "/tmp/audio.wav";
      case "list_prompt_history": return [];
      case "list_prompts": return [];
      case "search_prompts": return [];
      case "list_folders": return [];
      case "list_tags": return [];
      case "list_prompt_versions": return [];
      case "get_available_typing_tools": return [];
      case "get_available_accelerators": return { transcribe: [], ort: [], gpu_devices: [] };
      case "is_portable": return false;
      case "is_laptop": return false;
      case "get_secure_input_status": return { enabled:false, sustained:false, culprit_pid:null, culprit_name:null, fallback_active:false, covered_bindings:[], degraded_bindings:[], uncovered_bindings:[], recorder_blocked:false };
      case "get_windows_microphone_permission_status": return { supported:false, overall_access:"allowed", device_access:"allowed", app_access:"allowed", desktop_app_access:"allowed" };
      case "get_keyboard_implementation": return "handy_keys";
      case "harper_status": return { initialized:false, enabled:false };
      case "check_spelling": return [];
      case "os_speech_available": return false;
      case "os_speech_authorization_status": return "authorized";
      case "check_apple_intelligence_available": return false;
      case "is_recording": return false;
      case "get_microphone_mode": return false;
      case "get_model_load_status": return { is_loaded:false, current_model:null };
      case "initialize_enigo": return null;
      case "initialize_shortcuts": return null;
      case "show_main_window_command": return null;
      case "open_app_window": return null;
      case "open_speech_recognition_settings": return null;
      case "get_available_typing_tools": return [];
      default:
        return null;
    }
  }

  window.__TAURI_INTERNALS__.invoke = async (cmd, args, _opts) => mockInvoke(cmd, args);
  window.__TAURI_INTERNALS__.transformCallback = (cb, once=false) => {
    const id = nextId++;
    callbacks.set(id, (data) => {
      if (once) callbacks.delete(id);
      try { cb(data); } catch {}
    });
    return id;
  };
  window.__TAURI_INTERNALS__.unregisterCallback = (id) => callbacks.delete(id);
  window.__TAURI_INTERNALS__.runCallback = (id, data) => {
    const cb = callbacks.get(id);
    if (cb) cb(data);
  };
  window.__TAURI_INTERNALS__.callbacks = callbacks;
  window.__TAURI_INTERNALS__.metadata = { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } };
  window.__TAURI_INTERNALS__.convertFileSrc = (p, proto="asset") => proto + "://localhost/" + encodeURIComponent(p);
  window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener = () => {};
  try { window.isTauri = true; } catch {}
})();
`;
