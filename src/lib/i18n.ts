import { browser } from '$app/environment';
import { init, register, locale, waitLocale } from 'svelte-i18n';
import { invoke } from '@tauri-apps/api/core';

const defaultLocale = 'en';

register('en', () => import('./locales/en.json'));
register('ru', () => import('./locales/ru.json'));

// Initialize i18n and wait for it to be ready
async function initializeI18n() {
	await init({
		fallbackLocale: defaultLocale,
		initialLocale: defaultLocale,
	});
	
	// Wait for the initial locale to be loaded
	await waitLocale();
	
	// Now try to get system locale and update
	if (browser) {
		try {
			// Try to get locale from Tauri backend
			const systemLocale = await invoke<string>('get_system_locale');
			locale.set(systemLocale);
			await waitLocale(systemLocale);
		} catch (error) {
			console.warn('Failed to get system locale from backend:', error);
			// Fallback to browser locale
			const browserLocale = window.navigator.language.toLowerCase();
			const targetLocale = browserLocale.startsWith('ru') ? 'ru' : 'en';
			locale.set(targetLocale);
			await waitLocale(targetLocale);
		}
	}
}

export { initializeI18n };