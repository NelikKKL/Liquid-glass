// Тонкая обёртка над wasm-pack `--target web` сборкой.
// Использование (после сборки, см. README):
//
//   <script type="module">
//     import { LiquidGlass } from './js/liquid-glass.js';
//     await LiquidGlass.ready();
//     LiquidGlass.apply('.my-button');       // одна строка — и готово
//   </script>
//
// Или совсем декларативно — просто повесьте класс "liquid-glass" на элемент
// и вызовите LiquidGlass.autoInit() один раз после загрузки страницы.

import init, {
  liquidGlass,
  liquidGlassAll,
  autoInit as wasmAutoInit,
} from "../pkg/liquid_glass.js";

let initPromise = null;

function ready(wasmUrl) {
  if (!initPromise) {
    initPromise = init(wasmUrl);
  }
  return initPromise;
}

export const LiquidGlass = {
  ready,

  /** Применить эффект к первому элементу, подходящему под селектор. */
  apply(selector, options = {}) {
    return liquidGlass(selector, options);
  },

  /** Применить эффект ко всем элементам, подходящим под селектор. */
  applyAll(selector, options = {}) {
    return liquidGlassAll(selector, options);
  },

  /** Найти все `.liquid-glass` элементы на странице и включить эффект. */
  autoInit() {
    return wasmAutoInit();
  },
};

export default LiquidGlass;
