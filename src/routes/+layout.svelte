<script lang="ts">
	import '../app.css';
	import { initializeI18n } from '../lib/i18n';
	import { onMount } from 'svelte';
	import Console from '../lib/components/Console.svelte';
	import ConsoleToggle from '../lib/components/ConsoleToggle.svelte';
	import { consoleService } from '../lib/stores/console';

	let { children } = $props();
	let i18nReady = $state(false);

	onMount(async () => {
		await initializeI18n();
		// Initialize console service
		await consoleService.init();
		i18nReady = true;
	});
</script>

{#if i18nReady}
	{@render children()}
	
	<!-- Console components -->
	<Console />
	<ConsoleToggle />
{:else}
	<div class="loading">Loading...</div>
{/if}

<style>
	.loading {
		display: flex;
		justify-content: center;
		align-items: center;
		height: 100vh;
		font-size: 1.2rem;
	}
</style>
