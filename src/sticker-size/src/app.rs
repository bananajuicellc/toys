use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Event, HtmlInputElement, FileReader};

#[component]
pub fn App() -> impl IntoView {
    view! {
        <main class="container mx-auto p-4">
            <h1>"My PNG Utility App"</h1>
            
            // Plucked right out of the components folder:
            <FileInspector />
        </main>
    }
}

#[component]
pub fn FileInspector() -> impl IntoView {
    let (image_url, set_image_url) = signal(None::<String>);
    let (has_alpha, set_has_alpha) = signal(None::<bool>);
    let (error_msg, set_error_msg) = signal(None::<String>);

    let on_file_change = move |ev: Event| {
        let target = ev.target().unwrap().unchecked_into::<HtmlInputElement>();
        if let Some(files) = target.files() {
            if let Some(file) = files.get(0) {
                let file_reader = FileReader::new().unwrap();
                let fr_c = file_reader.clone();
                let file_name = file.name();

                let onload = move |_| {
                    let array_buffer = fr_c.result().unwrap();
                    let uint8_array = js_sys::Uint8Array::new(&array_buffer);
                    let bytes = uint8_array.to_vec();

                    set_image_url.set(Some(crate::sprite_inspector::to_data_uri(&file_name, &bytes)));
                    match crate::sprite_inspector::has_transparency(&bytes) {
                        Ok(result) => set_has_alpha.set(Some(result)),
                        Err(err) => set_error_msg.set(Some(err)),
                    }
                };

                let closure = wasm_bindgen::closure::Closure::wrap(Box::new(onload) as Box<dyn FnMut(Event)>);
                file_reader.set_onload(Some(closure.as_ref().unchecked_ref()));
                closure.forget();

                file_reader.read_as_array_buffer(&file).unwrap();
            }
        }
    };

    view! {
        <div>
            <input type="file" accept="image/png,image/gif" on:change=on_file_change />
            {move || has_alpha.get().map(|alpha| {
                if alpha {
                    view! { <p style="color: green">"Transparency detected!"</p> }
                } else {
                    view! { <p style="color: gray">"Image is fully opaque."</p> }
                }
            })}
            {move || error_msg.get().map(|err| view! { <p style="color: red">{err}</p> })}
            <Show
              when=move || image_url.get().is_some()
              fallback=|| view! { "Select an image." }
            >
                <img src=image_url />
            </Show>
        </div>
    }
}
