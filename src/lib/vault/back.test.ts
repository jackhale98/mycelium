import { describe, expect, it } from 'vitest';
import { decideBack } from './back';

describe('decideBack', () => {
	it('closes an overlay before anything else', () => {
		expect(decideBack('/vault/agenda', true)).toBe('dismiss');
		// Even at the root, an open sheet claims the press rather than exiting.
		expect(decideBack('/vault', true)).toBe('dismiss');
	});

	it('retraces a step from anywhere inside the app', () => {
		for (const path of ['/vault/agenda', '/vault/settings', '/vault/node/abc', '/vault/daily']) {
			expect(decideBack(path, false)).toBe('navigate');
		}
	});

	it('lets Android close the app from the top of the stack', () => {
		// Refusing here is the bug where back appears to do nothing and the user
		// cannot leave without the app switcher.
		expect(decideBack('/', false)).toBe('exit');
		expect(decideBack('/vault', false)).toBe('exit');
	});
});
