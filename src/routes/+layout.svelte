<script lang="ts">
	import '../app.css';
	import { initializeI18n } from '../lib/i18n';
	import { onMount } from 'svelte';

	let { children } = $props();
	let i18nReady = $state(false);

	onMount(async () => {
		await initializeI18n();
		i18nReady = true;
	});
</script>

{#if i18nReady}
	{@render children()}
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
