# minibro ᗢ

**minibro** es un asistente de escritorio nativo, minimalista y ultra-ligero que vive en la parte superior de la pantalla con apariencia de *mini-notch*.

Presenta una carita geométrica animada que parpadea y reacciona con distintos estados emocionales. Al interactuar con el notch, se expande un panel para supervisar tareas de agentes de inteligencia artificial (como Codex u OpenCode).

---

## Características actuales (MVP v0.1.0)

- **Wayland Layer Shell nativo:** Se acopla en la parte superior (`zwlr_layer_shell_v1`) sin barra de título ni decoraciones tradicionales en Hyprland / Wayland.
- **Renderizado geométrico Canvas:** Ojos y boca vectoriales con expresiones para 6 estados:
  - `Idle` (reposo con parpadeo periódico natural)
  - `Thinking` (oscilación y mirada atenta)
  - `Working` (pulso concentrado)
  - `Success` (expresión feliz `^ ^`)
  - `Waiting` (mirada neutra expectante)
  - `Error` (ojos en cruz `x x`)
- **Panel interactivo colapsable:** Expansión y contracción al hacer clic sobre el notch con tarjeta de tareas simuladas de agentes.
- **Consumo mínimo de CPU:** Suscripción de frames adaptativa (60 FPS durante parpadeos/animaciones, pausado a ~0% en reposo absoluto).
- **Arquitectura Hexagonal:** Dominio puro desacoplado del motor gráfico, listo para incorporar adaptadores de macOS (`NSPanel` / `cocoa`).
- **Runner Fallback:** Permite ejecutar como ventana transparente flotante mediante `--windowed` o en entornos sin Wayland Layer Shell.

---

## Requisitos

- **Rust:** Toolchain 1.80+ (edición 2024 recomendada).
- **Linux (Wayland):** Hyprland, Sway u otro compositor compatible con `zwlr_layer_shell_v1`.
- **Librerías del sistema:** `wayland-client`, `libxkbcommon`, `vulkan-loader`.

---

## Compilación y Ejecución

### Modo Layer Shell nativo (Wayland / Hyprland por defecto)
```bash
cargo run
```

### Modo ventana flotante (Fallback / Desarrollo / Cross-platform)
```bash
cargo run -- --windowed
```

### Ejecución sin dependencias Wayland (Prueba para macOS)
```bash
cargo run --no-default-features
```

---

## Pruebas y Calidad

```bash
cargo test
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
```

---

## Licencia

Distribuido bajo licencia MIT. Ver [LICENSE](LICENSE) para más detalles.
