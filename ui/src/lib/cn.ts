// Tiny className joiner — filters falsy parts so conditional classes stay terse.
export function cn(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(" ");
}
