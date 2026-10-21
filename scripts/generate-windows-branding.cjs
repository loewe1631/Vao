// Render the existing vector brand; no new artwork or external image service.
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');
const root = path.resolve(__dirname, '..');

function bitmap(width, height, rgba, icon = false) {
  const stride = icon ? width * 4 : Math.ceil(width * 3 / 4) * 4;
  const maskStride = Math.ceil(width / 32) * 4;
  const pixels = Buffer.alloc(stride * height + (icon ? maskStride * height : 0));
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const source = (y * width + x) * 4;
      const dest = (height - 1 - y) * stride + x * (icon ? 4 : 3);
      pixels[dest] = rgba[source + 2];
      pixels[dest + 1] = rgba[source + 1];
      pixels[dest + 2] = rgba[source];
      if (icon) {
        pixels[dest + 3] = rgba[source + 3];
        if (rgba[source + 3] === 0) {
          pixels[stride * height + (height - 1 - y) * maskStride + (x >> 3)] |= 0x80 >> (x & 7);
        }
      }
    }
  }
  const dib = Buffer.alloc(40);
  dib.writeUInt32LE(40, 0);
  dib.writeInt32LE(width, 4);
  dib.writeInt32LE(height * (icon ? 2 : 1), 8);
  dib.writeUInt16LE(1, 12);
  dib.writeUInt16LE(icon ? 32 : 24, 14);
  dib.writeUInt32LE(pixels.length, 20);
  if (icon) return Buffer.concat([dib, pixels]);
  const file = Buffer.alloc(14);
  file.write('BM');
  file.writeUInt32LE(14 + dib.length + pixels.length, 2);
  file.writeUInt32LE(54, 10);
  return Buffer.concat([file, dib, pixels]);
}

(async () => {
  const browser = await chromium.launch({ headless: true });
  try {
    const page = await browser.newPage();
    const svg = fs.readFileSync(path.join(root, 'ui/assets/logo.svg'), 'utf8');
    const rendered = await page.evaluate(async svg => {
      const image = new Image();
      image.src = 'data:image/svg+xml;base64,' + btoa(unescape(encodeURIComponent(svg)));
      await image.decode();
      const render = (width, height, draw) => {
        const canvas = document.createElement('canvas');
        canvas.width = width; canvas.height = height;
        const ctx = canvas.getContext('2d');
        draw(ctx);
        return { width, height, rgba: Array.from(ctx.getImageData(0, 0, width, height).data) };
      };
      const icons = [16, 24, 32, 48, 64, 128, 256].map(size =>
        render(size, size, ctx => ctx.drawImage(image, 0, 0, size, size)));
      const header = render(150, 57, ctx => {
        ctx.fillStyle = '#FFFFFF'; ctx.fillRect(0, 0, 150, 57);
        ctx.drawImage(image, 98, 6, 44, 44);
      });
      const sidebar = render(164, 314, ctx => {
        ctx.fillStyle = '#F2F7FF'; ctx.fillRect(0, 0, 164, 314);
        ctx.drawImage(image, 34, 72, 96, 96);
        ctx.fillStyle = '#14345C'; ctx.font = '600 30px Arial'; ctx.textAlign = 'center';
        ctx.fillText('Vao', 82, 210);
      });
      return { icons, header, sidebar };
    }, svg);
    const frames = rendered.icons.map(frame => bitmap(frame.width, frame.height, frame.rgba, true));
    const directory = Buffer.alloc(6 + 16 * frames.length);
    directory.writeUInt16LE(1, 2); directory.writeUInt16LE(frames.length, 4);
    let offset = directory.length;
    frames.forEach((frame, i) => {
      const size = rendered.icons[i].width;
      const entry = 6 + 16 * i;
      directory[entry] = size === 256 ? 0 : size;
      directory[entry + 1] = size === 256 ? 0 : size;
      directory.writeUInt16LE(1, entry + 4); directory.writeUInt16LE(32, entry + 6);
      directory.writeUInt32LE(frame.length, entry + 8); directory.writeUInt32LE(offset, entry + 12);
      offset += frame.length;
    });
    fs.writeFileSync(path.join(root, 'desktop/icons/windows.ico'), Buffer.concat([directory, ...frames]));
    for (const name of ['header', 'sidebar']) {
      const frame = rendered[name];
      fs.writeFileSync(path.join(root, `desktop/icons/windows-${name}.bmp`), bitmap(frame.width, frame.height, frame.rgba));
    }
    console.log('Generated Vao Windows icon (16–256px), header and sidebar.');
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
