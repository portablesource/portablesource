<script lang="ts">
	import { locale } from 'svelte-i18n';

	function changeLanguage(lang: string) {
		$locale = lang;
	}

	function handleLanguageChange(e: Event) {
		const target = e.target as HTMLSelectElement;
		if (target) {
			changeLanguage(target.value);
		}
	}

	const availableLocales = [
		{ code: 'en', name: 'English' },
		{ code: 'ru', name: 'Русский' }
	];
</script>

<div class="language-switcher">
	<select 
		value={$locale} 
		on:change={handleLanguageChange}
		class="px-3 py-1 border border-gray-300 rounded-md text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
	>
		{#each availableLocales as lang}
			<option value={lang.code}>{lang.name}</option>
		{/each}
	</select>
</div>

<style>
	.language-switcher {
		display: inline-block;
	}
</style>