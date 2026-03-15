export type ThemeMode = 'light' | 'dark' | 'system';

class ThemeStore {
	mode = $state<ThemeMode>('system');

	constructor() {
		if (typeof localStorage !== 'undefined') {
			const saved = localStorage.getItem('mycelium-theme') as ThemeMode | null;
			if (saved && ['light', 'dark', 'system'].includes(saved)) {
				this.mode = saved;
			}
		}
	}

	get isDark(): boolean {
		if (this.mode === 'system') {
			if (typeof window !== 'undefined') {
				return window.matchMedia('(prefers-color-scheme: dark)').matches;
			}
			return false;
		}
		return this.mode === 'dark';
	}

	setMode(mode: ThemeMode) {
		this.mode = mode;
		if (typeof localStorage !== 'undefined') {
			localStorage.setItem('mycelium-theme', mode);
		}
		this.applyTheme();
	}

	cycle() {
		const modes: ThemeMode[] = ['light', 'dark', 'system'];
		const idx = modes.indexOf(this.mode);
		this.setMode(modes[(idx + 1) % modes.length]);
	}

	applyTheme() {
		if (typeof document === 'undefined') return;
		const isDark = this.isDark;
		document.documentElement.classList.toggle('dark', isDark);
		document.documentElement.style.colorScheme = isDark ? 'dark' : 'light';
	}
}

export const theme = new ThemeStore();
