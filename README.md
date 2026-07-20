# Liquid Glass

Эффект "жидкого стекла" (Apple-style glassmorphism: преломление живого фона,
блики, кромка, 3D-наклон, реакция на курсор) для любого HTML-элемента. Ядро
написано на Rust и компилируется в WebAssembly.

**Итог сборки — ровно два файла**, оба генерирует `wasm-pack` автоматически:
`liquid_glass.js` (готовый ES-модуль с публичным API) и `liquid_glass_bg.wasm`.
Никакой дополнительной ручной JS-обёртки в проекте нет и не нужно — всё
публичное API (`LiquidGlass`, `liquidGlass`, `liquidGlassAll`, `autoInit`,
сеттеры) экспортируется прямо из Rust через `wasm-bindgen`.

## Почему быстро и без html2canvas

- **Реальное преломление живого фона** делает нативный CSS
  `backdrop-filter: blur(...) url(#svg-filter)`. SVG-фильтр
  (`feTurbulence` + `feDisplacementMap`), который Rust генерирует при
  инициализации, физически смещает пиксели фона под элементом — это
  выполняется композитором браузера на GPU. Работает "из коробки" для
  видео, CSS-анимаций, канвасов и текстовых анимаций — ничего не нужно
  вручную регистрировать, в отличие от подходов на `html2canvas`.
- **WebGL2-канвас** поверх элемента рисует только то, что нельзя получить
  из CSS: кромку (bevel), блики (specular), лёгкое "дыхание" поверхности.
  Один fullscreen quad, один fragment shader на элемент.
- **Скролл и ресайз** синхронизированы бесплатно: канвас — обычный дочерний
  элемент (`position:absolute; inset:0`) внутри самой цели, поэтому
  анимации самого элемента (в т.ч. GSAP/CSS transitions/scroll-driven)
  всегда синхронны без дополнительных scroll-листенеров.

## Реализованные возможности

| Фича                                    | Есть | Как реализовано                                   |
|------------------------------------------|:----:|-----------------------------------------------------|
| Real-time Refraction (static content)     | ✅   | SVG `feDisplacementMap` внутри `backdrop-filter`    |
| Real-time Refraction (video)              | ✅   | То же — работает автоматически, без регистрации     |
| Real-time Refraction (text animations)    | ✅   | То же — рефракция на уровне композитора браузера    |
| Real-time Refraction (CSS animations)     | ✅   | То же                                                |
| Magnification Control                     | ✅   | `magnify` / `setMagnify()`                          |
| Adjustable Bevel                          | ✅   | `bevelWidth` + `bevelDepth` / `setBevel()`           |
| Frosted Glass Effect                      | ✅   | `blur` / `setBlur()`                                 |
| Dynamic Shadows                           | ✅   | `shadow` / `setShadow()`, тень реагирует на наклон   |
| Specular Highlights                       | ✅   | `specular` / `setSpecular()`                         |
| Interactive Tilt Effect                   | ✅   | `tilt` + `tiltFactor` / `setTilt()`                  |
| Dynamic Element Support                   | ✅   | эффект живёт на реальном DOM-элементе                |
| GSAP-Ready Animations                     | ✅   | канвас — дочерний элемент, следует за transform      |
| Lightweight & Performant                  | ✅   | 1 shader + 1 CSS-фильтр на элемент, без скриншотов   |
| Seamless Scroll Sync                      | ✅   | канвас позиционируется CSS, а не JS-скроллом         |
| Auto-Resize Handling                      | ✅   | размер канваса пересчитывается в рендер-цикле        |
| Animate Lenses                            | ✅   | все параметры меняются "на лету" через сеттеры       |
| `on.init` / `onInit` callback             | ✅   | вызывается после первого отрисованного кадра         |

## Сборка

Нужны `rustup` (target `wasm32-unknown-unknown`) и `wasm-pack`:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack

wasm-pack build --target web --release --out-dir pkg
```

Это создаст папку `pkg/` с **двумя** артефактами — `liquid_glass.js` и
`liquid_glass_bg.wasm` (плюс типы `.d.ts`) — и больше ничего подключать не
нужно. GitHub Action в этом репозитории делает то же самое автоматически
при каждом пуше (см. `.github/workflows/build.yml`), прикладывает `pkg/`
как артефакт сборки, а на тег `v*` — публикует релиз (и, при наличии
секрета `NPM_TOKEN`, пакет в npm).

## Использование

Подключаем **только** результат сборки — никакого второго JS-файла:

### Декларативно (data-атрибуты)

```html
<button class="my-button liquid-glass" data-lg-tint="#88ccff" data-lg-blur="16">
  Купить
</button>

<script type="module">
  import init, { autoInit } from "./pkg/liquid_glass.js";
  await init(); // подгружает liquid_glass_bg.wasm
  autoInit();   // подхватит все .liquid-glass элементы на странице
</script>
```

Полный список data-атрибутов: `data-lg-intensity`, `data-lg-tint`,
`data-lg-blur`, `data-lg-radius`, `data-lg-bevel-width`,
`data-lg-bevel-depth`, `data-lg-magnify`, `data-lg-tilt`,
`data-lg-tilt-factor`, `data-lg-shadow`, `data-lg-specular`,
`data-lg-interactive`.

### Императивно

```js
import init, { liquidGlass } from "./pkg/liquid_glass.js";

await init();

const glass = liquidGlass(".my-button", {
  intensity: 1.0,     // сила бликов/преломления, 0–2
  bevelWidth: 14,      // px, ширина кромки
  bevelDepth: 1.0,      // 0–1, интенсивность кромки
  blur: 16,             // backdrop-filter blur, px
  tint: "#ffffff",      // оттенок стекла
  interactive: true,    // блик следует за курсором мыши
  magnify: 1.2,          // 0.001–3.0, искажение фона под стеклом
  tilt: true,            // 3D-наклон от курсора
  tiltFactor: 6,          // градусы наклона
  shadow: true,            // динамическая тень
  specular: true,           // блики
  onInit: (target) => console.log("готово", target),
});

// позже, при необходимости:
glass.setIntensity(1.6);
glass.setTint("#ffd166");
glass.setMagnify(1.8);
glass.setTilt(true, 10);
glass.setBevel(20, 0.8);
glass.setBlur(24);
glass.setShadow(false);
glass.setSpecular(false);
glass.destroy();
```

`liquidGlassAll(selector, options)` применяет эффект сразу ко всем
подходящим элементам и возвращает массив хэндлов.

## Опции конструктора

| Опция         | По умолчанию            | Описание                                        |
|---------------|--------------------------|--------------------------------------------------|
| `radius`      | `border-radius` элемента | Скругление углов стекла, px                      |
| `bevelWidth`  | `14`                      | Ширина кромки/блика по краю, px                  |
| `bevelDepth`  | `1.0`                     | Интенсивность кромки, 0–1                        |
| `blur`        | `16`                      | Сила `backdrop-filter` размытия фона, px         |
| `intensity`   | `1.0`                     | Общая сила бликов и преломления                  |
| `tint`        | `"#ffffff"`               | Оттенок стекла (hex)                              |
| `interactive` | `true`                    | Блик реагирует на положение курсора              |
| `magnify`     | `1.0`                     | Искажение фона под стеклом, 0.001–3.0            |
| `tilt`        | `false`                   | 3D-наклон стекла от положения курсора            |
| `tiltFactor`  | `6`                       | Глубина наклона, градусы                         |
| `shadow`      | `true`                    | Динамическая тень под стеклом                    |
| `specular`    | `true`                    | Анимированные блики                              |
| `onInit`      | `—`                       | `(targetElement) => void`, после первого кадра   |

## Демо

Откройте `www/index.html` через любой статический сервер (нужен, так как
`fetch()` WASM-модуля не работает с `file://`):

```bash
wasm-pack build --target web --release --out-dir pkg
npx serve .
# затем открыть /www/index.html
```

## Структура проекта

```
liquid-glass/
├── src/lib.rs              # Rust/WebGL2 + SVG-рефракция — единственный источник логики
├── Cargo.toml
├── www/index.html           # демо-страница, подключает только pkg/
├── pkg/                     # генерируется wasm-pack (в git не хранится): liquid_glass.js + liquid_glass_bg.wasm
└── .github/workflows/build.yml
```
