

# blippy

GitHub en tu terminal.

blippy es una TUI (interfaz de terminal) centrada en el teclado para gestionar incidencias y pull requests de GitHub.

https://github.com/user-attachments/assets/14daa99b-c39a-43d9-b32d-9d5a6840819f

Consulta la [demostración completa de características](DEMO.md) para ver más capturas de pantalla.

## Requisitos

- Cadena de herramientas de Rust (`1.93+` recomendada) para compilaciones desde código fuente
- GitHub CLI (`gh`) es altamente recomendado para la mejor experiencia de trabajo (mecanismo alternativo de autenticación, checkout de PR y una integración más fluida con GitHub)
- Soporte para llaveros del sistema operativo (macOS Keychain, Administrador de credenciales de Windows, Secret Service de Linux)

## Instalación

### npm (global)

```bash
npm i -g blippy
```

### Homebrew

```bash
brew install AksharP5/tap/blippy
```

### Instalador por shell (macOS/Linux)

```bash
curl -fsSL https://github.com/AksharP5/blippy/releases/latest/download/blippy-installer.sh | bash
```

### Instalador por PowerShell (Windows)

```powershell
irm https://github.com/AksharP5/blippy/releases/latest/download/blippy-installer.ps1 | iex
```

### Compilar desde el código fuente

```bash
cargo install --git https://github.com/AksharP5/blippy
```

## Comandos CLI

- `blippy`: inicia la TUI
- `blippy --version`: muestra información de la versión
- `blippy sync`: escanea repositorios locales y almacena en caché los remotos de GitHub
- `blippy auth reset`: elimina el token de autenticación almacenado del llavero
- `blippy cache reset`: elimina la base de datos de caché local

## Qué Puedes Hacer

- Explorar y gestionar incidencias y pull requests
- Crear incidencias desde la TUI con un paso de confirmación
- Abrir incidencias/PRs vinculados en la TUI o en el navegador
- Revisar diffs de PR con comentarios en línea y resolución de hilos
- Fusionar pull requests desde la TUI cuando los permisos del repositorio lo permitan
- Distinguir pull requests fusionados de los cerrados
- Editar etiquetas y responsables (cuando los permisos del repositorio lo permitan)
- Personalizar temas, atajos de teclado y plantillas de comentarios para cerrar

Consulta [FEATURES.md](FEATURES.md) para un desglose completo de las características.

## Teclado y Ratón

- blippy prioriza flujos de trabajo con teclado para mayor fiabilidad
- Existe soporte para ratón/pantalla táctil, pero puede ser inconsistente
- Referencia completa de teclas: [KEYBINDS.md](KEYBINDS.md)

## Configuración

- Archivo de configuración: `~/.config/blippy/config.toml`
- Sobrescritura de atajos: `~/.config/blippy/keybinds.toml`
- Archivo de ejemplo de atajos: [keybinds.example.toml](keybinds.example.toml)

Ejemplo de tema:

```toml
theme = "midnight"
```

Temas integrados disponibles:

- `github_dark` (predeterminado)
- `midnight`
- `graphite`

Ejemplo de plantilla de comentario:

```toml
[[comment_defaults]]
name = "close_default"
body = "Closing this issue as resolved."
```

## Documentación

- Demostración de características con capturas de pantalla: [DEMO.md](DEMO.md)
- Autenticación y configuración de PAT: [AUTH.md](AUTH.md)
- Guía de características: [FEATURES.md](FEATURES.md)
- Referencia de teclas: [KEYBINDS.md](KEYBINDS.md)
- Guía de contribución: [CONTRIBUTING.md](CONTRIBUTING.md)
- Historial de versiones: [CHANGELOG.md](CHANGELOG.md)
