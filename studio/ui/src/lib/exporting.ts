// Exporting (#112): the files an export makes and their names, and PNG
// drawn from the core's SVG in the webview, so the studio needs no
// raster library. Pure, but for `rasterize`.

import type { Ground } from './backend';

export type Format = 'svg' | 'png' | 'ir' | 'zip';

/** The PNG sizes offered, as Preview's and the common icon sizes. */
export const PNG_SIZES = [16, 24, 32, 48, 64, 128, 256] as const;

/** An icon's file: `x.svg` and `x@dark.svg`, `x@16.png` and
    `x@16-dark.png`. An icon's name never holds `@` (it is a file name of
    letters, digits, `-`, `_` and `.`), so no two names meet. */
export function fileName(icon: string, ground: Ground, px?: number): string {
  if (px === undefined) return ground === 'dark' ? `${icon}@dark.svg` : `${icon}.svg`;
  return ground === 'dark' ? `${icon}@${px}-dark.png` : `${icon}@${px}.png`;
}

/** `name` in `folder`, with the folder's own separator. */
export function inFolder(folder: string, name: string): string {
  const sep = folder.includes('\\') && !folder.includes('/') ? '\\' : '/';
  return `${folder.replace(/[\\/]+$/, '')}${sep}${name}`;
}

/** The viewBox an SVG declares, or none. */
export function viewBoxOf(svg: string): [number, number, number, number] | null {
  const m = /viewBox="([^"]+)"/.exec(svg);
  const v = m?.[1].trim().split(/[\s,]+/).map(Number);
  return v && v.length === 4 && v.every(Number.isFinite) ? (v as [number, number, number, number]) : null;
}

/** A PNG of size `px` for an icon of `viewbox`: its longer side `px`, as
    Preview fits an icon in a square, the other in proportion. */
export function pngSize(viewbox: [number, number, number, number], px: number): { width: number; height: number } {
  const [, , w, h] = viewbox;
  const other = (a: number, b: number) => Math.max(1, Math.round((px * a) / b));
  return w >= h ? { width: px, height: other(h, w) } : { width: other(w, h), height: px };
}

/** `svg` with its own width and height set to these, so the browser draws
    it at that size rather than scaling a smaller drawing up. */
export function sized(svg: string, width: number, height: number): string {
  return svg.replace(/<svg\b[^>]*>/, (tag) => {
    const bare = tag.replace(/\s(width|height)="[^"]*"/g, '');
    return bare.replace(/^<svg\b/, `<svg width="${width}" height="${height}"`);
  });
}

/** `svg` drawn as a PNG of `width` × `height`, in the webview. */
export async function rasterize(svg: string, width: number, height: number): Promise<Uint8Array> {
  const url = URL.createObjectURL(new Blob([sized(svg, width, height)], { type: 'image/svg+xml' }));
  try {
    const img = new Image();
    img.src = url;
    await img.decode();
    const canvas = document.createElement('canvas');
    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('this window cannot draw a PNG');
    ctx.drawImage(img, 0, 0, width, height);
    const blob = await new Promise<Blob | null>((done) => canvas.toBlob(done, 'image/png'));
    if (!blob) throw new Error('this window cannot draw a PNG');
    return new Uint8Array(await blob.arrayBuffer());
  } finally {
    URL.revokeObjectURL(url);
  }
}
