package coupe_caddy_plugin

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"

	"github.com/caddyserver/caddy/v2"
	"github.com/caddyserver/caddy/v2/caddyconfig/caddyfile"
	"github.com/caddyserver/caddy/v2/caddyconfig/httpcaddyfile"
	"github.com/caddyserver/caddy/v2/modules/caddyhttp"
	"go.uber.org/zap"
)

func init() {
	httpcaddyfile.RegisterHandlerDirective("coupe", parseCaddyfileHandler)
	caddy.RegisterModule(CoupeMiddleware{})
}

type CoupeMiddleware struct {
	FunctionName    string
	SessionDuration time.Duration
	client          *http.Client
	logger          *zap.Logger
}

func (CoupeMiddleware) CaddyModule() caddy.ModuleInfo {
	return caddy.ModuleInfo{
		ID:  "http.handlers.coupe",
		New: func() caddy.Module { return new(CoupeMiddleware) },
	}
}

func (m *CoupeMiddleware) Provision(ctx caddy.Context) error {
	m.logger = ctx.Logger()
	m.client = http.DefaultClient
	return nil
}

type StartSessionRequest struct {
	FunctionName    string `json:"function_name"`
	DurationSeconds int    `json:"duration_seconds"`
}

func (m CoupeMiddleware) ServeHTTP(w http.ResponseWriter, r *http.Request, next caddyhttp.Handler) error {
	m.logger.Info("starting session",
		zap.String("function_name", m.FunctionName),
		zap.Duration("session_duration", m.SessionDuration))

	body := StartSessionRequest{
		FunctionName:    m.FunctionName,
		DurationSeconds: int(m.SessionDuration.Seconds()),
	}
	bodyBytes, err := json.Marshal(body)
	if err != nil {
		m.logger.Error("error marshalling body", zap.Error(err))
		return fmt.Errorf("failed to marshal request body: %w", err)
	}

	req, err := http.NewRequest("POST", "http://sentinel:8081/sessions", bytes.NewBuffer(bodyBytes))
	if err != nil {
		m.logger.Error("error creating request", zap.Error(err))
		return fmt.Errorf("failed to create request: %w", err)
	}

	for k, v := range r.Header {
		req.Header[k] = v
	}
	req.Header.Set("Content-Type", "application/json")

	resp, err := m.client.Do(req)
	if err != nil {
		m.logger.Error("error making request", zap.Error(err))
		http.Error(w, err.Error(), http.StatusServiceUnavailable)
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		// Read error response body
		responseBody, err := io.ReadAll(resp.Body)
		if err != nil {
			m.logger.Error("error reading error response body", zap.Error(err))
			http.Error(w, err.Error(), http.StatusServiceUnavailable)
			return err
		}

		m.logger.Error("error response from sentinel",
			zap.Int("status_code", resp.StatusCode),
			zap.String("response_body", string(responseBody)))

		// Forward the original status code and response body
		w.WriteHeader(resp.StatusCode)
		w.Header().Set("Content-Type", "application/json")
		w.Write(responseBody)
		return fmt.Errorf("%s", string(responseBody))
	}

	return next.ServeHTTP(w, r)
}

func (m *CoupeMiddleware) UnmarshalCaddyfile(d *caddyfile.Dispenser) error {
	d.Next() // consume the directive name

	for nesting := d.Nesting(); d.NextBlock(nesting); {
		subdirective := d.Val()
		if !d.NextArg() {
			return d.ArgErr()
		}
		switch subdirective {
		case "function_name":
			m.FunctionName = d.Val()
		case "session_duration":
			dur, err := time.ParseDuration(d.Val())
			if err != nil {
				return d.Errf("error parsing session duration: %v", err)
			}
			m.SessionDuration = dur
		default:
			return d.Errf("unrecognized subdirective '%s'", subdirective)
		}
	}
	return nil
}

func parseCaddyfileHandler(h httpcaddyfile.Helper) (caddyhttp.MiddlewareHandler, error) {
	var m CoupeMiddleware
	err := m.UnmarshalCaddyfile(h.Dispenser)
	return m, err
}

// Interface guards
var (
	_ caddyfile.Unmarshaler       = (*CoupeMiddleware)(nil)
	_ caddy.Provisioner           = (*CoupeMiddleware)(nil)
	_ caddyhttp.MiddlewareHandler = (*CoupeMiddleware)(nil)
)
