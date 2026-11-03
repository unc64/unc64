# Fonts

Subsets of the UNC 64's fonts (fonts/), as WOFF2 for the Machine Room, made with fontTools:

    pyftsubset fonts/Anton-Regular.ttf --unicodes="U+0020-007E,U+00A0-00FF,U+2010-2027,U+2030-203A,U+2190-2193,U+2212" --flavor=woff2 --output-file=web/fonts/Anton-Regular.woff2

and the same for LibreBaskerville-Regular, -Bold and -Italic; UNC-Regular with --unicodes="*".

Anton (Vernon Adams) and Libre Baskerville (Impallari Type) are licensed under the SIL Open Font License 1.1 (https://openfontlicense.org). The OFL allows bundling them with software as long as the fonts aren't sold on their own.

UNC-Regular is the UNC 64's own character ROM (rom/unscii.txt, the UNSCII glyphs, public domain), made into a font by scripts/unc-font.py.
