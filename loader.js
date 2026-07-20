import init, { LiquidGlassRenderer } from '../pkg/liquid_glass.js';

async function setupLiquidGlass() {
  await init();

  const elements = document.querySelectorAll('[data-liquid-glass]');

  elements.forEach((el) => {
    // 1. Базовые CSS-стили для элемента
    el.style.position = 'relative';
    el.style.overflow = 'hidden';
    el.style.backdropFilter = 'blur(16px) saturate(180%)';
    el.style.webkitBackdropFilter = 'blur(16px) saturate(180%)';

    // 2. Создаем подложку-canvas
    const canvas = document.createElement('canvas');
    canvas.style.position = 'absolute';
    canvas.style.top = '0';
    canvas.style.left = '0';
    canvas.style.width = '100%';
    canvas.style.height = '100%';
    canvas.style.pointerEvents = 'none';
    canvas.style.zIndex = '0';

    el.insertBefore(canvas, el.firstChild);

    // Делаем контент кнопки выше канваса
    Array.from(el.children).forEach((child) => {
      if (child !== canvas) child.style.zIndex = '1';
    });

    const rect = el.getBoundingClientRect();
    canvas.width = rect.width * window.devicePixelRatio;
    canvas.height = rect.height * window.devicePixelRatio;

    const renderer = new LiquidGlassRenderer(canvas);

    // Рендер-цикл
    function loop() {
      renderer.render(canvas.width, canvas.height);
      requestAnimationFrame(loop);
    }
    requestAnimationFrame(loop);
  });
}

setupLiquidGlass();