import { describe, expect, it } from 'vitest';
import { appHeightFor } from './keyboardViewport';

describe('appHeightFor', () => {
	it('fits the app above an open keyboard', () => {
		expect(appHeightFor(452.4, 852)).toBe(452);
	});

	it('fills the screen when no keyboard is up', () => {
		expect(appHeightFor(852, 852)).toBeNull();
		// A collapsing browser toolbar is not a keyboard.
		expect(appHeightFor(800, 852)).toBeNull();
	});
});
