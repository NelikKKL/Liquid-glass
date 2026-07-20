# Liquid Glass

Эффект "жидкого стекла" (Apple-style glassmorphism: прозрачность, преломление
света, блики, реакция на курсор) для любого HTML-элемента. Ядро написано на
Rust и компилируется в WebAssembly, рендер — через WebGL2.

## Почему быстро

- Само размытие фона делает **нативный** `backdrop-filter: blur()` — это
  выполняется композитором браузера на GPU, без единой строчки JS-кода в
  рендер-цикле.
- WebGL2 рисует поверх элемента только динамическую часть (блики, кромку,
  преломление, хроматическую аберрацию) — один `fullscreen quad` и один
  fragment shader на элемент. Никакого захвата пикселей страницы (`html2canvas`
  и т.п.) не требуется.
- Позиционирование канваса синхронизировано с прокруткой через CSS
  `transform` внутри уже существующего `requestAnimationFrame`-цикла — без
  дополнительных scroll-листенеров и без layout thrashing.
- Ресайз WebGL-канваса (дорогая операция) происходит только при реальном
  изменении размера элемента, а не каждый кадр.

## Сборка

Нужны `rustup` (target `wasm32-unknown-unknown`) и `wasm-pack`:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack

wasm-pack build --target web --release --out-dir pkg
```

Это создаст папку `pkg/` с `liquid_glass.js`, `liquid_glass_bg.wasm` и
тайпингами. GitHub Action в этом репозитории делает то же самое
автоматически при каждом пуше (см. `.github/workflows/build.yml`) и
прикладывает `pkg/` как артефакт сборки, а на тег `v*` — публикует релиз
(и, при наличии секрета `NPM_TOKEN`, пакет в npm).

## Использование

### Декларативно (одна строка CSS-класса)

```html
<button class="my-button liquid-glass">Купить</button>

<script type="module">
  import { LiquidGlass } from "./js/liquid-glass.js";
  await LiquidGlass.ready(new URL("./pkg/liquid_glass_bg.wasm", import.meta.url));
  LiquidGlass.autoInit(); // подхватит все .liquid-glass элементы на странице
</script>
```

Параметры можно задать data-атрибутами:

```html
<div class="card liquid-glass"
     data-lg-intensity="1.2"
     data-lg-tint="#88ccff"
     data-lg-blur="20"
     data-lg-radius="24"
     data-lg-border="2">
  ...
</div>
```

### Императивно (JS-однострочник)

```js
import { LiquidGlass } from "./js/liquid-glass.js";

await LiquidGlass.ready(new URL("./pkg/liquid_glass_bg.wasm", import.meta.url));

const glass = LiquidGlass.apply(".my-button", {
  intensity: 1.0,   // сила бликов/преломления, 0–2
  border: 1.5,      // толщина кромки, px
  blur: 16,         // backdrop-filter blur, px
  tint: "#ffffff",  // оттенок стекла
  interactive: true // блик следует за курсором мыши
});

// позже, при необходимости:
glass.setIntensity(1.6);
glass.setTint("#ffd166");
glass.destroy();
```

`LiquidGlass.applyAll(selector, options)` применяет эффект сразу ко всем
подходящим элементам и возвращает массив хэндлов.

## Опции

| Опция         | По умолчанию            | Описание                                  |
|---------------|--------------------------|--------------------------------------------|
| `radius`      | `border-radius` элемента | Скругление углов стекла, px                |
| `border`      | `1.5`                     | Толщина кромки/блика по краю, px           |
| `blur`        | `16`                      | Сила `backdrop-filter` размытия фона, px   |
| `intensity`   | `1.0`                     | Общая сила бликов и преломления            |
| `tint`        | `"#ffffff"`               | Оттенок стекла (hex)                        |
| `interactive` | `true`                    | Блик реагирует на положение курсора        |

## Демо

Откройте `www/index.html` через любой статический сервер (нужен, так как
`fetch()` WASM-модуля не работает с `file://`):

```bash
npx serve .
```

## Структура проекта

```
liquid-glass/
├── src/lib.rs              # Rust/WebGL2 ядро эффекта
├── Cargo.toml
├── js/liquid-glass.js       # тонкая JS-обёртка с публичным API
├── www/index.html           # демо-страница
├── pkg/                     # генерируется wasm-pack (в git не хранится)
└── .github/workflows/build.yml
```
