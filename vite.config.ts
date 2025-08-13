import tailwindcss from '@tailwindcss/vite';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
	esbuild: {
		drop: process.env.NODE_ENV === 'production' ? ['console', 'debugger'] : []
	},
	build: {
		target: 'es2020',
		sourcemap: false,
		rollupOptions: {
			output: {
				manualChunks: {
					vendor: ['svelte', 'svelte-i18n', '@tauri-apps/api']
				}
			}
		}
	},
	server: {
		port: 1337,
		host: true,
		strictPort: true,
		proxy: {
			'/api': {
				target: 'https://portables.dev',
				changeOrigin: true,
				secure: true
			}
		}
	}
});
