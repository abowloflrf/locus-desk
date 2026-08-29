import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig, loadEnv } from 'vite';

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, '.', '');

  return {
    plugins: [tailwindcss(), svelte()],
    resolve: {
      alias: {
        $lib: new URL('./src/lib', import.meta.url).pathname,
      },
    },
    server: {
      host: '127.0.0.1',
      port: Number(env.VITE_DEV_PORT || 5173),
      strictPort: true,
      proxy: {
        '/api': env.VITE_API_TARGET || 'http://127.0.0.1:7310',
      },
    },
  };
});
