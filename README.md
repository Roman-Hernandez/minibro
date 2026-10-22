# minibro ᗢ

**minibro** es un asistente de escritorio nativo, minimalista y ultra-ligero que vive en la parte superior de la pantalla con apariencia de *mini-notch*.

Presenta una carita geométrica animada que reacciona a la actividad real de OpenCode. El panel permite conversar, revisar herramientas, responder preguntas y aprobar acciones del asistente.

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
- **OpenCode como motor:** Instancia local propia, conversación y actividad reales, preguntas, permisos de una sola ejecución, cancelación y restauración de la última conversación. Reutiliza la configuración de OpenCode.
- **Panel colapsable y ampliable:** Conversación, actividad y ajustes; avisos flotantes cuando hace falta responder, al finalizar o ante un error. Contraer no cancela ni aprueba acciones.
- **Posicionamiento:** Notch compacto de 168 × 32 unidades lógicas, arrastre horizontal y botones de monitor `1`, `2`… El clic se confirma al soltar; movimientos menores a 6 unidades no inician un arrastre.
- **Posición persistente:** Guarda monitor y posición horizontal al terminar el arrastre o elegir pantalla. Recupera una pantalla disponible si falta la guardada.
- **Animación adaptativa:** Actualizaciones de unos 60 FPS durante animaciones; en reposo espera hasta el próximo parpadeo, sin sondeo continuo de monitores.
- **Arquitectura Hexagonal:** Dominio puro, gestos y geometría compartidos; APIs de plataforma aisladas en infraestructura.
- **Runner Fallback:** Permite ejecutar como ventana transparente flotante mediante `--windowed` o en entornos sin Wayland Layer Shell.

---

## Requisitos

- **Rust:** Toolchain estable actualizado, compatible con la edición 2024. La versión mínima soportada todavía no está validada.
- **Linux (Wayland):** Hyprland, Sway u otro compositor compatible con `zwlr_layer_shell_v1`.
- **Librerías del sistema:** `wayland-client`, `libxkbcommon`, `vulkan-loader`.
- **OpenCode instalado y configurado:** Contrato de referencia probado con 1.19.4. Las credenciales y el modelo se configuran en OpenCode; minibro no lo instala ni guarda sus credenciales. En Linux el selector de carpeta usa el portal de escritorio.

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

Linux Layer Shell y macOS comparten el asistente, su cliente OpenCode, las vistas y el posicionamiento. macOS usa el runner de ventana y AppKit para consultar pantallas; la validación gráfica de esta nueva integración sigue pendiente. En el runner convencional de otras plataformas se conserva el arrastre del sistema, sin prometer selección de monitor ni restauración de posición. Las ventanas Wayland convencionales no permiten posicionamiento absoluto desde el cliente.

El arrastre entre monitores no forma parte de este incremento: usar los botones numerados para cambiar de pantalla. La posición se guarda en `$XDG_STATE_HOME/minibro/position.json` (por defecto `~/.local/state/minibro/position.json`) en Linux y en `~/Library/Application Support/minibro/position.json` en macOS.

Para empezar, abre **Settings**, elige la carpeta de trabajo y conecta OpenCode. Si el ejecutable no está disponible desde una aplicación gráfica, selecciónalo en esa misma vista. Puedes pedir, por ejemplo, «crea un proyecto y ábrelo en VS Code»; las acciones y errores reales aparecen en **Activity**. `Enter` envía y `Shift+Enter` inserta una línea.

Ver [integración, validación y límites de OpenCode](docs/opencode-integration.md). Aprobar sesiones de otros terminales es una integración futura independiente.

Ver [diseño y validación de posicionamiento](docs/notch-positioning.md) para límites y pruebas de plataforma.

---

## Pruebas y Calidad

```bash
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo build --locked
```

Repetir Clippy, tests y build con `--no-default-features` para validar el runner fallback. La CI valida también esta configuración en macOS.

Las reglas de contribución y el flujo de ramas están en [AGENTS.md](AGENTS.md).

---

## Licencia

Distribuido bajo licencia MIT. Ver [LICENSE](LICENSE) para más detalles.
