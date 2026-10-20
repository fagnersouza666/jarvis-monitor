import { copyFileSync, mkdirSync } from 'node:fs';
mkdirSync('web-dist', { recursive: true });
for (const file of ['index.html', 'style.css']) copyFileSync(`ui/${file}`, `web-dist/${file}`);
