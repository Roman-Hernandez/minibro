# minibro ᗢ

**minibro** es un asistente de escritorio nativo, minimalista y ultra-ligero que vive en la parte superior de la pantalla con apariencia de *mini-notch*.

Presenta una carita geométrica animada que reacciona a la actividad real del motor local de **OpenCode**. Su interfaz permite conversar en streaming con soporte para Markdown, seguir la actividad y llamadas a herramientas en tiempo real, responder preguntas interactivas del agente, aprobar permisos de ejecución y gestionar la configuración en una ventana flotante independiente.

---

## Características principales

- **Wayland Layer Shell nativo:** Se acopla en la parte superior (`zwlr_layer_shell_v1`) como un notch sutil y elegante, sin barra de título ni decoraciones tradicionales en Hyprland, Sway y otros compositores compatibles.
- **Renderizado geométrico Canvas:** Avatar vectorial animado que reacciona con expresiones para 6 estados reales:
  - `Idle`: reposo con parpadeo periódico natural (sin sondeo continuo).
  - `Thinking`: oscilación y mirada atenta mientras el modelo procesa la respuesta.
  - `Working`: pulso concentrado mientras se ejecutan herramientas y comandos.
  - `Success`: expresión feliz (`^ ^`) al completar la tarea satisfactoriamente.
  - `Waiting`: mirada neutra expectante ante una pregunta o solicitud de permiso pendiente.
  - `Error`: ojos en cruz (`x x`) ante fallos de conexión o errores del motor.
- **OpenCode como cerebro:**
  - Instancia local propia gestionada por minibro con ciclo de vida seguro.
  - Reutiliza modelos, credenciales, MCPs y reglas de permisos configuradas en OpenCode.
  - Soporte completo de Server-Sent Events (SSE) para respuestas fluidas en streaming y registro de actividad en vivo.
  - Renderizado de Markdown enriquecido en las respuestas del asistente.
  - Peticiones de permisos («Permitir una vez» / «Rechazar») y preguntas interactivas (opciones simples, selección múltiple y texto personalizado).
  - Cancelación/Stop limpio que aborta operaciones en curso en el motor.
  - Restauración automática del último espacio de trabajo y sesión al iniciar.
- **Panel notch minimalista:**
  - Pestañas simplificadas: **Actividad** (vista por defecto al expandir) y **Chat**.
  - Cierre automático al hacer clic fuera del panel (*click-outside dismiss*).
  - Tamaño normal (520 × 560 px lógicos) y ampliable (760 × 720 px lógicos).
  - Avisos flotantes (*floating notices*) compactos bajo el notch para requerir atención inmediata o reportar finalización/error.
- **Ventana independiente de Ajustes (`⚙`):**
  - Ventana flotante desacoplada del notch para seleccionar carpeta de trabajo, ejecutable de OpenCode, verificar estado de conexión y abrir nueva conversación.
- **Posicionamiento inteligente y multi-monitor:**
  - Notch compacto de 168 × 32 unidades lógicas.
  - Arrastre horizontal con umbral de activación (6 unidades) para evitar arrastres accidentales.
  - Selector de monitores mediante botones compactos (`1`, `2`…) con tooltips informativos del nombre de pantalla.
  - Persistencia atómica de monitor y posición en disco (`position.json`).
- **Arquitectura Hexagonal limpia:**
  - Núcleo de dominio y lógica de aplicación independientes de Iced, Wayland o sistemas operativos.
  - Soporte multiplataforma con runner nativo Wayland y runner fallback/macOS en una única base de código.

---

## Requisitos

- **Rust:** Toolchain estable (Rust 2024 edition).
- **Linux (Wayland):** Hyprland, Sway o cualquier compositor compatible con `zwlr_layer_shell_v1`.
- **Librerías del sistema:** `wayland-client`, `libxkbcommon`, `vulkan-loader`.
- **OpenCode:** Probado con la versión **1.19.4** o superior instalada en el sistema. Configura tus proveedores y modelos previamente en OpenCode; minibro reutiliza tu configuración existente sin manipular credenciales ni requerir claves directamente.
  - *Linux:* El selector de carpetas utiliza el portal de escritorio XDG (`xdg-desktop-portal`).

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

## Integración con OpenCode

### Configuración y Puesta en Marcha
1. Inicia minibro y haz clic en el icono **`⚙`** en el notch para abrir la ventana flotante de ajustes.
2. Selecciona la **carpeta de trabajo** para la sesión (el directorio raíz del proyecto).
3. Si el ejecutable de OpenCode no está en el `PATH` estándar del entorno gráfico, selecciónalo manualmente o define la variable de entorno `MINIBRO_OPENCODE_BIN`.
4. Pulsa **«Connect / check status»**. minibro iniciará un subproceso local de OpenCode vinculado exclusivamente a `127.0.0.1` con un puerto explícito y contraseña aleatoria en memoria, sin exponer puertos al exterior.
5. Puedes iniciar un nuevo contexto en cualquier momento con el botón **«New conversation»**.

### Flujo de Trabajo
- **Chat:** Conversación con el modelo. Admite saltos de línea con `Shift+Enter` y envío directo con `Enter`. Las respuestas se formatean en Markdown en tiempo real.
- **Actividad:** Registro detallado de herramientas invocadas, comandos ejecutados y resultados reportados por el motor.
- **Preguntas y Aprobaciones:** Cuando el agente solicita ejecutar comandos o modificar archivos que requieren permiso, o cuando formula una pregunta, aparece una tarjeta interactiva tanto en el chat/actividad como en el aviso flotante. Aprobar o rechazar responde directamente a OpenCode.
- **Parada (Stop):** Durante la ejecución activa, el botón de detención solicita a OpenCode interrumpir la tarea.

### Almacenamiento, Límites y Seguridad
- **Estado de sesión:** Se guarda en `assistant.json` junto a `position.json` (`$XDG_STATE_HOME/minibro/` en Linux, `~/Library/Application Support/minibro/` en macOS). Solo almacena la ruta de la carpeta, el ejecutable y el ID de sesión raíz de minibro. **No almacena credenciales ni transcripciones**.
- **Seguridad:** minibro no ejecuta comandos de shell por su cuenta; todo comando es procesado y ejecutado por OpenCode bajo sus políticas de seguridad.
- **Límites defensivos:** 64 KiB en salida de arranque, marcos SSE acotados a 1 MiB, respuestas JSON hasta 16 MiB, y hasta 256 mensajes/actividades recientes en memoria.
- **Aislamiento:** minibro gestiona únicamente su propio árbol de sesiones hijas; las sesiones externas iniciadas en terminales independientes no son modificadas ni finalizadas.

---

## Posicionamiento y Multi-Monitor

- **Dimensiones:** El notch cerrado mide 168 × 32 unidades lógicas; el panel desplegado mide 520 × 560 (normal) o 760 × 720 (modo ampliado), adaptándose automáticamente a pantallas más pequeñas.
- **Arrastre horizontal:** Arrastra el notch horizontalmente a lo largo del borde superior de la pantalla. Un filtro de jitter ignora desplazamientos menores a 6 píxeles antes de iniciar el arrastre, evitando aperturas accidentales.
- **Cambio de pantalla:** Usa los botones numerados (`1`, `2`…) en la cabecera para transferir el notch de monitor. Cada botón muestra el nombre de la salida (p. ej. `eDP-1`, `HDMI-A-1`) al pasar el cursor.
- **Persistencia atómica:** La posición horizontal y el monitor seleccionado se guardan de forma atómica en disco (`position.json`). Si una pantalla desconectada ya no está disponible, minibro se recupera automáticamente centrándose en una pantalla activa sin perder la preferencia original.

---

## Pruebas y Calidad

Para validar el formato, linting y pruebas unitarias:

```bash
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo build --locked
```

Verificación del runner fallback y compilación para macOS:

```bash
cargo clippy --locked --no-default-features --all-targets -- -D warnings
cargo test --locked --no-default-features --all-targets
cargo build --locked --no-default-features
```

### Pruebas de integración con OpenCode real (Opcional)

Las pruebas unitarias por defecto utilizan mocks locales y no requieren conexión con OpenCode. Si deseas ejecutar las pruebas de integración reales con el binario de OpenCode instalado:

```bash
MINIBRO_TEST_OPENCODE=/ruta/a/opencode cargo test --locked reference_server_contract_and_shutdown -- --ignored
MINIBRO_TEST_OPENCODE=/ruta/a/opencode cargo test --locked real_engine_creates_only_the_approved_temporary_project -- --ignored
```

---

## Arquitectura

El proyecto sigue una estricta **Arquitectura Hexagonal**:
- `domain`: Lógica pura del asistente y del personaje animado (estados, parpadeos, panel), completamente desacoplada de Iced, sistemas de ventanas o almacenamiento.
- `application`: Coordinación de casos de uso, posicionamiento, gestos de arrastre y control de estado de la interacción.
- `presentation`: Widgets de Iced, canvas del avatar, panel de conversación/actividad, ventana de ajustes y renderizado de Markdown.
- `infrastructure`: Adaptadores del motor OpenCode (proceso, cliente SSE, serialización), almacenamiento atómico en disco y runners de escritorio (`runner_wayland` con Layer Shell y `runner_fallback` multi-ventana).

---

## Licencia

Distribuido bajo licencia MIT. Ver [LICENSE](LICENSE) para más detalles.
