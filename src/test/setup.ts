import '@testing-library/jest-dom';
import { vi } from 'vitest';

// Mock WebGL and HTMLCanvasElement methods for jsdom
HTMLCanvasElement.prototype.getContext = vi.fn().mockImplementation((type) => {
  if (type === 'webgl' || type === 'webgl2' || type === 'experimental-webgl') {
    return {
      viewport: vi.fn(),
      clearColor: vi.fn(),
      clear: vi.fn(),
      enable: vi.fn(),
      disable: vi.fn(),
      createBuffer: vi.fn(),
      bindBuffer: vi.fn(),
      bufferData: vi.fn(),
      createProgram: vi.fn(),
      createShader: vi.fn(),
      shaderSource: vi.fn(),
      compileShader: vi.fn(),
      attachShader: vi.fn(),
      linkProgram: vi.fn(),
      useProgram: vi.fn(),
      getAttribLocation: vi.fn().mockReturnValue(0),
      getUniformLocation: vi.fn().mockReturnValue({}),
      enableVertexAttribArray: vi.fn(),
      vertexAttribPointer: vi.fn(),
      drawArrays: vi.fn(),
      drawElements: vi.fn(),
      createTexture: vi.fn(),
      bindTexture: vi.fn(),
      texParameteri: vi.fn(),
      texImage2D: vi.fn(),
      generateMipmap: vi.fn(),
      getExtension: vi.fn().mockReturnValue(null),
      getParameter: vi.fn().mockImplementation((param) => {
        if (param === 7938) return 'WebGL 1.0'; // VERSION
        if (param === 35724) return 'WebGL GLSL ES 1.0'; // SHADING_LANGUAGE_VERSION
        if (param === 7937) return 'WebKit WebGL'; // RENDERER
        if (param === 7936) return 'WebKit'; // VENDOR
        if (param === 34047) return 16; // MAX_TEXTURE_SIZE
        if (param === 34921) return 16; // MAX_VERTEX_ATTRIBS
        if (param === 35661) return 32; // MAX_COMBINED_TEXTURE_IMAGE_UNITS
        return 'WebGL 1.0';
      }),
      getShaderPrecisionFormat: vi.fn().mockReturnValue({ precision: 23, rangeMin: 127, rangeMax: 127 }),
    };
  }
  return null;
});

// Mock Tauri API invoke/listen globally for frontend tests
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockImplementation(async (cmd) => {
    if (cmd === 'get_settings') {
      return {
        download_directory: '/downloads',
        max_concurrent_tasks: 2,
        skip_previously_downloaded: false,
        cookies_browser: '',
        yt_dlp_channel: 'Stable',
        advanced_mode: false,
        custom_yt_dlp_flags: '',
        custom_yt_dlp_path: '',
      };
    }
    if (cmd === 'get_queue') return [];
    if (cmd === 'get_history') return [];
    if (cmd === 'run_self_check') {
      return {
        yt_dlp: { status: 'OK', version: '2026.03.17' },
        ffmpeg: { status: 'OK' },
        database: 'OK',
        bin_dir: '/bin',
      };
    }
    return {};
  }),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockImplementation(async () => {
    return () => {};
  }),
}));
