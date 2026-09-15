import "server-only";

/**
 * Server-side API client.
 *
 * Server Components talk to the Rust API over the internal compose network; the browser
 * talks to the same API through Caddy (or the dev rewrite) at a same-origin `/api/v1`, so
 * nothing needs a public API host.
 */
const baseUrl = process.env.API_INTERNAL_URL ?? "http://api:8080";

export class ApiError extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

type FetchOptions = RequestInit & {
  /** Seconds to cache; omit for always-fresh (the default for live data). */
  revalidate?: number;
};

export async function apiFetch<T>(path: string, options: FetchOptions = {}): Promise<T> {
  const { revalidate, ...init } = options;
  const response = await fetch(`${baseUrl}/api/v1${path}`, {
    ...init,
    headers: { "content-type": "application/json", ...(init.headers ?? {}) },
    cache: revalidate === undefined ? "no-store" : undefined,
    next: revalidate === undefined ? undefined : { revalidate },
  });

  if (!response.ok) {
    const body = await response.text();
    let message = body;
    try {
      message = (JSON.parse(body) as { error?: string }).error ?? body;
    } catch {
      // Not JSON: fall back to the raw body, trimmed by the caller if needed.
    }
    throw new ApiError(message || response.statusText, response.status);
  }

  return (await response.json()) as T;
}

/** `undefined` instead of throwing, for pages that render their own empty state. */
export async function apiFetchOptional<T>(
  path: string,
  options: FetchOptions = {},
): Promise<T | undefined> {
  try {
    return await apiFetch<T>(path, options);
  } catch (error) {
    if (error instanceof ApiError && error.status === 404) {
      return undefined;
    }
    throw error;
  }
}
