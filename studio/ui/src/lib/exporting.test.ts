import { describe, expect, it } from 'vitest';
import { fileName, inFolder, pngSize, sized, viewBoxOf } from './exporting';

describe('exported files', () => {
  it('are named by icon, ground and size, and no two meet', () => {
    expect(fileName('rack', 'light')).toBe('rack.svg');
    expect(fileName('rack', 'dark')).toBe('rack@dark.svg');
    expect(fileName('rack', 'light', 16)).toBe('rack@16.png');
    expect(fileName('rack', 'dark', 16)).toBe('rack@16-dark.png');
    // an icon named as another's dark file still has a name of its own
    expect(fileName('rack-dark', 'light')).not.toBe(fileName('rack', 'dark'));
  });

  it('go into a folder with its own separator', () => {
    expect(inFolder('C:\\out\\', 'a.svg')).toBe('C:\\out\\a.svg');
    expect(inFolder('/home/me/out', 'a.svg')).toBe('/home/me/out/a.svg');
    expect(inFolder('/home/me/out/', 'a.svg')).toBe('/home/me/out/a.svg');
  });
});

describe('a PNG', () => {
  it('has the size on its longer side, the other in proportion', () => {
    expect(pngSize([0, 0, 64, 48], 32)).toEqual({ width: 32, height: 24 });
    expect(pngSize([-5, -5, 30, 60], 32)).toEqual({ width: 16, height: 32 });
    expect(pngSize([0, 0, 40, 40], 128)).toEqual({ width: 128, height: 128 });
    expect(pngSize([0, 0, 1000, 1], 16)).toEqual({ width: 16, height: 1 });
  });

  it('is drawn from the SVG at that size, its viewBox kept', () => {
    const svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="-1.50 0.00 64.00 48.00" width="64.00" height="48.00">\n<title>x</title></svg>\n';
    expect(viewBoxOf(svg)).toEqual([-1.5, 0, 64, 48]);
    expect(sized(svg, 32, 24)).toBe('<svg width="32" height="24" xmlns="http://www.w3.org/2000/svg" viewBox="-1.50 0.00 64.00 48.00">\n<title>x</title></svg>\n');
    expect(viewBoxOf('<svg/>')).toBeNull();
  });
});
