import tailwindcss from '@tailwindcss/vite';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
	server: {
		port: 1337,
		host: true,
		proxy: {
			'/api': {
				target: 'https://portables.dev',
				changeOrigin: true,
				secure: true
			}
		}
	}
});
