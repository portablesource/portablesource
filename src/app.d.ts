// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		// interface PageData {}
		// interface PageState {}
		// interface Platform {}
	}

	// Tauri API types
	interface Window {
		__TAURI__?: {
			process: {
				exit(code?: number): Promise<void>;
			};
			// Add other Tauri APIs as needed
		};
	}
}

export {};
