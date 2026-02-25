interface Env {
  GOOGLE_CLIENT_ID: string;
  GOOGLE_CLIENT_SECRET: string;
  HMAC_SECRET: string;
}

async function hmacVerify(
  payload: string,
  signature: string,
  secret: string,
): Promise<boolean> {
  const encoder = new TextEncoder();
  const key = await crypto.subtle.importKey(
    "raw",
    encoder.encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["verify"],
  );
  const sigBytes = Uint8Array.from(
    atob(signature.replace(/-/g, "+").replace(/_/g, "/")),
    (c) => c.charCodeAt(0),
  );
  return crypto.subtle.verify("HMAC", key, sigBytes, encoder.encode(payload));
}

function errorPage(message: string): Response {
  return new Response(
    `<!DOCTYPE html>
<html><body>
<h1>Authentication failed</h1>
<p>${message}</p>
<p>Please close this window and try again.</p>
</body></html>`,
    { status: 400, headers: { "Content-Type": "text/html" } },
  );
}

export const onRequestGet: PagesFunction<Env> = async (context) => {
  const url = new URL(context.request.url);
  const code = url.searchParams.get("code");
  const state = url.searchParams.get("state");
  const error = url.searchParams.get("error");

  if (error) {
    return errorPage(`Google returned an error: ${error}`);
  }

  if (!code || !state) {
    return errorPage("Missing code or state parameter.");
  }

  // Parse and verify HMAC state: port:timestamp:nonce:signature
  const parts = state.split(":");
  if (parts.length !== 4) {
    return errorPage("Invalid state parameter.");
  }

  const [port, timestamp, nonce, signature] = parts;
  const payload = `${port}:${timestamp}:${nonce}`;

  const valid = await hmacVerify(payload, signature, context.env.HMAC_SECRET);
  if (!valid) {
    return errorPage("Invalid state signature.");
  }

  // Check timestamp is within 10 minutes
  const stateTime = parseInt(timestamp, 10);
  const now = Math.floor(Date.now() / 1000);
  if (Math.abs(now - stateTime) > 600) {
    return errorPage("Authentication request expired. Please try again.");
  }

  // Exchange code for tokens
  const tokenResponse = await fetch("https://oauth2.googleapis.com/token", {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({
      code,
      client_id: context.env.GOOGLE_CLIENT_ID,
      client_secret: context.env.GOOGLE_CLIENT_SECRET,
      redirect_uri: "https://caldir.org/auth/google/callback",
      grant_type: "authorization_code",
    }),
  });

  if (!tokenResponse.ok) {
    const errorText = await tokenResponse.text();
    console.error("Token exchange failed:", errorText);
    return errorPage("Failed to exchange authorization code for tokens.");
  }

  const tokens = (await tokenResponse.json()) as {
    access_token: string;
    refresh_token: string;
    expires_in: number;
  };

  // Redirect tokens to local CLI listener
  const callbackParams = new URLSearchParams({
    access_token: tokens.access_token,
    refresh_token: tokens.refresh_token,
    expires_in: tokens.expires_in.toString(),
  });

  return Response.redirect(
    `http://localhost:${port}/callback?${callbackParams.toString()}`,
    302,
  );
};
