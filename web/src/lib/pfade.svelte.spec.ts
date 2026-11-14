/**
 * Lokale Pfade sind keine Links, und der Kontoname steht
 * nirgends im Bild (Links-Kästen, /pc). Erfundene Namen.
 */
import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import {
	PROFIL_PLATZHALTER,
	SATZ_LOKALER_PFAD,
	VOLUME_PLATZHALTER,
	istLokalerPfad,
	profilVerbergen
} from './pfade.ts';
import LinksListe from './components/projekt/LinksListe.svelte';

describe('istLokalerPfad', () => {
	it('erkennt Laufwerk, file: und Netzwerkpfad', () => {
		for (const p of [
			'C:\\Users\\beispiel\\Downloads\\x.zip',
			'C:/Users/beispiel/Projekte/bericht',
			'd:\\Ablage',
			'file:///C:/Ablage/x.pdf',
			'FILE://rechner/x',
			'\\\\rechner\\freigabe\\x.pdf',
			'  C:\\mit-leerzeichen-vorn'
		]) {
			expect(istLokalerPfad(p), p).toBe(true);
		}
	});
	it('Web-Adressen, mailto und Wege im Dashboard bleiben Links', () => {
		for (const p of [
			'https://example.org/C:/x',
			'http://localhost:8787/',
			'mailto:a@example.org',
			'/projects/x',
			'C:'
		]) {
			expect(istLokalerPfad(p), p).toBe(false);
		}
	});
});

describe('istLokalerPfad unter macOS und Linux', () => {
	it('erkennt Profil- und Systemformen', () => {
		for (const p of [
			'/Users/beispiel/Downloads/x.zip',
			'/home/beispiel/dev',
			'/Volumes/Platte/Ablage',
			'/root',
			'/usr/local/bin/git',
			'/opt/homebrew/bin/node',
			'/private/var/folders/ab/x/T',
			'/Applications',
			'  /Users/beispiel'
		]) {
			expect(istLokalerPfad(p), p).toBe(true);
		}
	});
	it('ein Weg im Dashboard mit / vorn ist nie lokal', () => {
		for (const p of [
			'/projekte/x',
			'/projects/x',
			'/pc',
			'/daten',
			'/anstehend',
			'/',
			'/Userspace/x',
			'/homepage',
			'/users/beispiel',
			'https://example.org/Users/beispiel',
			'ssh://host/Users/beispiel/repo.git'
		]) {
			expect(istLokalerPfad(p), p).toBe(false);
		}
	});
});

describe('profilVerbergen unter macOS und Linux', () => {
	it('ersetzt das Profil, behält den Rest', () => {
		expect(profilVerbergen('/Users/beispiel/Downloads/x.zip')).toBe(
			`${PROFIL_PLATZHALTER}/Downloads/x.zip`
		);
		expect(profilVerbergen('/home/beispiel')).toBe(PROFIL_PLATZHALTER);
		expect(profilVerbergen('/System/Volumes/Data/Users/beispiel/x')).toBe(
			`${PROFIL_PLATZHALTER}/x`
		);
		expect(profilVerbergen('/Volumes/Platte/Users/beispiel/x')).toBe(`${PROFIL_PLATZHALTER}/x`);
		expect(profilVerbergen('file:///Users/beispiel/x.pdf')).toBe(`${PROFIL_PLATZHALTER}/x.pdf`);
		expect(profilVerbergen('/root/.cargo/bin')).toBe(`${PROFIL_PLATZHALTER}/.cargo/bin`);
		expect(profilVerbergen('/var/root')).toBe(PROFIL_PLATZHALTER);
	});
	it('mehrere Stellen in einem Befehl, Anführungszeichen bleiben', () => {
		expect(profilVerbergen('"/Users/beispiel/bin/a" --ort /home/beispiel/b')).toBe(
			`"${PROFIL_PLATZHALTER}/bin/a" --ort ${PROFIL_PLATZHALTER}/b`
		);
	});
	it('der Name eines Datenträgers wird ersetzt', () => {
		expect(profilVerbergen('/Volumes/Beispiels-Platte/Fotos')).toBe(
			`/Volumes/${VOLUME_PLATZHALTER}/Fotos`
		);
	});
	it('zwei Pfade hintereinander bleiben zwei Pfade', () => {
		expect(profilVerbergen('ls /Users/beispiel /Users/beispiel2')).toBe(
			`ls ${PROFIL_PLATZHALTER} ${PROFIL_PLATZHALTER}`
		);
		expect(profilVerbergen('/home/a;/home/b')).toBe(`${PROFIL_PLATZHALTER};${PROFIL_PLATZHALTER}`);
		expect(profilVerbergen('/Users/beispiel, /home/beispiel2/x')).toBe(
			`${PROFIL_PLATZHALTER}, ${PROFIL_PLATZHALTER}/x`
		);
		// Ein Name mit Leerzeichen gilt nur, wenn danach ein Trenner kommt.
		expect(profilVerbergen('/Users/Beispiel Konto/x')).toBe(`${PROFIL_PLATZHALTER}/x`);
		expect(profilVerbergen('/Users/beispiel ist kaputt')).toBe(`${PROFIL_PLATZHALTER} ist kaputt`);
	});
	it('eine Liste mit Doppelpunkt (PATH) wird je Pfad geschwärzt', () => {
		expect(profilVerbergen('PATH=/usr/bin:/Users/beispiel/.cargo/bin')).toBe(
			`PATH=/usr/bin:${PROFIL_PLATZHALTER}/.cargo/bin`
		);
		expect(profilVerbergen('/usr/bin:/home/beispiel/bin')).toBe(
			`/usr/bin:${PROFIL_PLATZHALTER}/bin`
		);
		expect(profilVerbergen('PATH=/Users/beispiel/bin:/usr/bin:/home/beispiel2/bin:/root/bin')).toBe(
			`PATH=${PROFIL_PLATZHALTER}/bin:/usr/bin:${PROFIL_PLATZHALTER}/bin:${PROFIL_PLATZHALTER}/bin`
		);
		expect(profilVerbergen('/usr/bin:/Volumes/Platte/x')).toBe(
			`/usr/bin:/Volumes/${VOLUME_PLATZHALTER}/x`
		);
	});
	it('file: mit einem, zwei oder drei Schrägstrichen und mit localhost', () => {
		expect(profilVerbergen('file:/Users/beispiel/x.pdf')).toBe(`${PROFIL_PLATZHALTER}/x.pdf`);
		expect(profilVerbergen('file://Users/beispiel/x.pdf')).toBe(`${PROFIL_PLATZHALTER}/x.pdf`);
		expect(profilVerbergen('file:///home/beispiel')).toBe(PROFIL_PLATZHALTER);
		expect(profilVerbergen('file:/root/x')).toBe(`${PROFIL_PLATZHALTER}/x`);
		expect(profilVerbergen('file:///Volumes/Platte/Fotos')).toBe(
			`file:///Volumes/${VOLUME_PLATZHALTER}/Fotos`
		);
		expect(profilVerbergen('file://localhost/Users/beispiel/x')).toBe(
			`file://localhost${PROFIL_PLATZHALTER}/x`
		);
		expect(profilVerbergen('FILE://LOCALHOST/home/beispiel')).toBe(
			`FILE://LOCALHOST${PROFIL_PLATZHALTER}`
		);
	});
	it('Schrägstriche wie in JSON (\\/)', () => {
		expect(profilVerbergen('"\\/Users\\/beispiel\\/x"')).toBe(`"${PROFIL_PLATZHALTER}\\/x"`);
		expect(profilVerbergen('"\\/home\\/beispiel"')).toBe(`"${PROFIL_PLATZHALTER}"`);
		// Gegenprobe: eine Domäne davor bleibt eine Adresse.
		expect(profilVerbergen('example.org\\/home\\/beispiel')).toBe('example.org\\/home\\/beispiel');
	});
	it('Adressen im Netz, Git-Remotes und andere Pfade bleiben', () => {
		for (const p of [
			'https://example.org/users/beispiel',
			'https://example.org/home/beispiel/x',
			'ssh://host/Users/beispiel/repo.git',
			'git@host:/Users/beispiel/repo.git',
			'host.example:/home/beispiel/repo.git',
			'ssh://konto@host:22/home/beispiel/repo.git',
			'https://example.org/ab:/Users/beispiel',
			'http://users/beispiel',
			'/Users',
			'/home/',
			'/rootfs/x',
			'/var/rootless',
			'/usr/local/bin/git',
			'/projects/beispiel',
			'/Volumes/'
		]) {
			expect(profilVerbergen(p), p).toBe(p);
		}
	});
});

describe('profilVerbergen', () => {
	it('ersetzt das Profil, behält den Rest', () => {
		expect(profilVerbergen('C:\\Users\\beispiel\\Downloads\\x.zip')).toBe(
			`${PROFIL_PLATZHALTER}\\Downloads\\x.zip`
		);
		expect(profilVerbergen('c:/users/Beispiel Konto/Projekte')).toBe(
			`${PROFIL_PLATZHALTER}/Projekte`
		);
		expect(profilVerbergen('D:\\Benutzer\\beispiel')).toBe(PROFIL_PLATZHALTER);
		expect(profilVerbergen('file:///C:/Users/beispiel/x.pdf')).toBe(`${PROFIL_PLATZHALTER}/x.pdf`);
	});
	it('mehrere Stellen in einem Befehl, Anführungszeichen bleiben', () => {
		expect(
			profilVerbergen('"C:\\Users\\beispiel\\AppData\\a.exe" --ort C:\\Users\\beispiel\\b')
		).toBe(`"${PROFIL_PLATZHALTER}\\AppData\\a.exe" --ort ${PROFIL_PLATZHALTER}\\b`);
	});
	it('ein Laufwerk ist ein einzelner Buchstabe an einer Wortgrenze', () => {
		// Gegenprobe zum Laufwerk: der letzte Buchstabe eines Worts vor „:“ ist keins.
		for (const p of [
			'host:/Users/beispiel/x',
			'git@host:/users/beispiel/repo.git',
			'ab:\\Users\\x'
		]) {
			expect(profilVerbergen(p), p).toBe(p);
		}
		expect(profilVerbergen('nach C:\\Users\\beispiel\\x')).toBe(`nach ${PROFIL_PLATZHALTER}\\x`);
		expect(profilVerbergen('"C:/Users/beispiel"')).toBe(`"${PROFIL_PLATZHALTER}"`);
	});
	it('ein Laufwerk beginnt auch nach einem Escape oder einem kodierten Zeichen', () => {
		expect(profilVerbergen('Fehler:\\nC:\\\\Users\\\\beispiel\\\\x')).toBe(
			`Fehler:\\n${PROFIL_PLATZHALTER}\\\\x`
		);
		expect(profilVerbergen('%22C:/Users/beispiel/x')).toBe(`%22${PROFIL_PLATZHALTER}/x`);
		expect(profilVerbergen('x\\tC:\\Users\\beispiel')).toBe(`x\\t${PROFIL_PLATZHALTER}`);
	});
	it('andere Pfade bleiben, wie sie sind', () => {
		for (const p of [
			'C:\\ProgramData\\Dashboard\\dienst.toml',
			'C:\\Users',
			'D:/Ablage/Projekte',
			'kein Pfad'
		]) {
			expect(profilVerbergen(p)).toBe(p);
		}
	});
});

describe('LinksListe', () => {
	it('lokaler Pfad: Text mit Hinweis, ohne Link und ohne Kontonamen', () => {
		const { container } = render(LinksListe, {
			links: [
				{ name: 'Export', url: 'C:\\Users\\beispiel\\Downloads\\export.zip' },
				{ name: 'Ablage', url: 'file:///D:/Ablage' },
				{ name: 'Netz', url: 'https://example.org/seite' }
			]
		});
		const links = [...container.querySelectorAll('a')];
		expect(links.map((a) => a.textContent)).toEqual(['Netz']);
		expect(links[0]?.getAttribute('href')).toBe('https://example.org/seite');
		expect(screen.getByText('Export').closest('a')).toBeNull();
		expect(container.textContent).not.toContain('beispiel');
		expect(container.textContent).toContain(`${PROFIL_PLATZHALTER}\\Downloads\\export.zip`);
		expect(screen.getAllByText(SATZ_LOKALER_PFAD)).toHaveLength(2);
	});
});
