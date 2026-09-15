// Command adapter-go is the Go implementation of the PQ-CAS prober contract.
//
// Unlike the CLI-driven adapters it uses crypto/tls directly, which is the point: Go's own
// TLS stack is one of the columns of the compatibility matrix, and shelling out to a
// command would measure something else. crypto/tls has no public knob for the
// signature_algorithms extension, so this adapter pins the key-exchange group only and says
// so in the report's diagnostics rather than pretending otherwise.
package main

import (
	"crypto/tls"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net"
	"net/http"
	"os"
	"sort"
	"strings"
	"time"
)

type target struct {
	Host     string `json:"host"`
	Port     int    `json:"port"`
	SNI      string `json:"sni,omitempty"`
	HTTPPath string `json:"http_path,omitempty"`
}

type captureOptions struct {
	RawRecords bool `json:"raw_records"`
	Keylog     bool `json:"keylog"`
}

type probeRequest struct {
	Target       target         `json:"target"`
	KEMGroups    []string       `json:"kem_groups"`
	SigAlgs      []string       `json:"sig_algs"`
	TLSVersions  []string       `json:"tls_versions"`
	CipherSuites []string       `json:"cipher_suites"`
	TimeoutMS    int            `json:"timeout_ms"`
	Capture      captureOptions `json:"capture"`
}

type messageField struct {
	Label  string   `json:"label"`
	Values []string `json:"values"`
}

type handshakeMessage struct {
	Index     uint32         `json:"index"`
	Direction string         `json:"direction"`
	Kind      string         `json:"kind"`
	Fields    []messageField `json:"fields"`
}

type alert struct {
	Level       string `json:"level"`
	Description string `json:"description"`
}

type negotiated struct {
	TLSVersion      *string `json:"tls_version,omitempty"`
	CipherSuite     *string `json:"cipher_suite,omitempty"`
	Group           *string `json:"group,omitempty"`
	CertVerifySigAl *string `json:"cert_verify_sig_alg,omitempty"`
}

type certificate struct {
	Subject            string  `json:"subject"`
	Issuer             string  `json:"issuer"`
	NotBefore          *string `json:"not_before,omitempty"`
	NotAfter           *string `json:"not_after,omitempty"`
	SignatureAlgorithm *string `json:"signature_algorithm,omitempty"`
	PublicKeyAlgorithm *string `json:"public_key_algorithm,omitempty"`
}

type httpOutcome struct {
	Status      int            `json:"status"`
	Headers     []messageField `json:"headers"`
	BodyExcerpt *string        `json:"body_excerpt,omitempty"`
}

type evidence struct {
	BytesSent     uint64  `json:"bytes_sent"`
	BytesReceived uint64  `json:"bytes_received"`
	UDSDumpB64    *string `json:"uds_dump_b64,omitempty"`
}

type probeResponse struct {
	Adapter          string             `json:"adapter"`
	HandshakeSuccess bool               `json:"handshake_success"`
	Negotiated       negotiated         `json:"negotiated"`
	Messages         []handshakeMessage `json:"messages"`
	Alert            *alert             `json:"alert,omitempty"`
	CertChain        []certificate      `json:"cert_chain"`
	HTTP             *httpOutcome       `json:"http,omitempty"`
	FailedStage      string             `json:"failed_stage"`
	ErrorSummary     *string            `json:"error_summary,omitempty"`
	Diagnostics      any                `json:"diagnostics"`
	Evidence         evidence           `json:"evidence"`
	DurationMS       uint64             `json:"duration_ms"`
}

type capabilities struct {
	Adapter     string   `json:"adapter"`
	Version     string   `json:"version"`
	KEMGroups   []string `json:"kem_groups"`
	SigAlgs     []string `json:"sig_algs"`
	TLSVersions []string `json:"tls_versions"`
}

// groupByName maps the catalog spelling (and the common aliases the other stacks use) onto
// the CurveIDs Go actually implements.
var groupByName = map[string]tls.CurveID{
	"x25519":         tls.X25519,
	"X25519":         tls.X25519,
	"P-256":          tls.CurveP256,
	"secp256r1":      tls.CurveP256,
	"P-384":          tls.CurveP384,
	"secp384r1":      tls.CurveP384,
	"P-521":          tls.CurveP521,
	"secp521r1":      tls.CurveP521,
	"X25519MLKEM768": tls.X25519MLKEM768,
	"x25519MLKEM768": tls.X25519MLKEM768,
}

func supportedGroups() []string {
	names := make([]string, 0, len(groupByName))
	seen := map[tls.CurveID]bool{}
	for name, id := range groupByName {
		if seen[id] {
			continue
		}
		seen[id] = true
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}

// Go chooses signature algorithms itself; these are the ones it will accept from a peer.
func supportedSigAlgs() []string {
	return []string{
		"ecdsa_secp256r1_sha256", "ecdsa_secp384r1_sha384", "ecdsa_secp521r1_sha512",
		"ed25519", "rsa_pss_rsae_sha256", "rsa_pss_rsae_sha384", "rsa_pss_rsae_sha512",
		"rsa_pkcs1_sha256", "rsa_pkcs1_sha384", "rsa_pkcs1_sha512",
	}
}

func env(key, fallback string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return fallback
}

func str(s string) *string { return &s }

func versionName(v uint16) string {
	switch v {
	case tls.VersionTLS13:
		return "TLSv1.3"
	case tls.VersionTLS12:
		return "TLSv1.2"
	case tls.VersionTLS11:
		return "TLSv1.1"
	case tls.VersionTLS10:
		return "TLSv1.0"
	default:
		return fmt.Sprintf("0x%04x", v)
	}
}

func groupName(id tls.CurveID) string {
	for name, candidate := range groupByName {
		if candidate == id && name == strings.ToUpper(name[:1])+name[1:] {
			return name
		}
	}
	for name, candidate := range groupByName {
		if candidate == id {
			return name
		}
	}
	return id.String()
}

// Go surfaces alerts as text in the error string; translate the ones students will meet.
func alertFromError(err error) *alert {
	msg := strings.ToLower(err.Error())
	switch {
	case strings.Contains(msg, "handshake failure"):
		return &alert{Level: "fatal", Description: "handshake_failure"}
	case strings.Contains(msg, "bad record mac"), strings.Contains(msg, "decryption failed"):
		return &alert{Level: "fatal", Description: "bad_record_mac"}
	case strings.Contains(msg, "protocol version"):
		return &alert{Level: "fatal", Description: "protocol_version"}
	case strings.Contains(msg, "illegal parameter"):
		return &alert{Level: "fatal", Description: "illegal_parameter"}
	case strings.Contains(msg, "unknown certificate authority"), strings.Contains(msg, "unknown ca"):
		return &alert{Level: "fatal", Description: "unknown_ca"}
	default:
		return nil
	}
}

func clientHello(req probeRequest) handshakeMessage {
	versions := req.TLSVersions
	if len(versions) == 0 {
		versions = []string{"TLSv1.3", "TLSv1.2"}
	}
	return handshakeMessage{
		Index:     0,
		Direction: "outbound",
		Kind:      "ClientHello",
		Fields: []messageField{
			{Label: "Offered Groups", Values: req.KEMGroups},
			{Label: "Signature Algorithms", Values: req.SigAlgs},
			{Label: "Supported Versions", Values: versions},
			{Label: "Cipher Suites Offered", Values: []string{
				"TLS_AES_128_GCM_SHA256", "TLS_AES_256_GCM_SHA384", "TLS_CHACHA20_POLY1305_SHA256",
			}},
		},
	}
}

func probe(req probeRequest, adapterName string) probeResponse {
	started := time.Now()
	timeout := time.Duration(req.TimeoutMS) * time.Millisecond
	if timeout <= 0 {
		timeout = 15 * time.Second
	}

	resp := probeResponse{
		Adapter:     adapterName,
		Messages:    []handshakeMessage{clientHello(req)},
		CertChain:   []certificate{},
		FailedStage: "tls_handshake",
		Diagnostics: map[string]any{
			"offeredGroups":  req.KEMGroups,
			"offeredSigAlgs": req.SigAlgs,
			"note":           "crypto/tls does not expose signature_algorithms selection; groups are pinned, signature algorithms are advisory",
		},
	}

	var curves []tls.CurveID
	var unknown []string
	for _, name := range req.KEMGroups {
		if id, ok := groupByName[name]; ok {
			curves = append(curves, id)
		} else {
			unknown = append(unknown, name)
		}
	}
	if len(curves) == 0 && len(req.KEMGroups) > 0 {
		resp.ErrorSummary = str(fmt.Sprintf("crypto/tls does not implement: %s", strings.Join(unknown, ", ")))
		resp.DurationMS = uint64(time.Since(started).Milliseconds())
		return resp
	}

	sni := req.Target.SNI
	if sni == "" {
		sni = req.Target.Host
	}
	address := net.JoinHostPort(req.Target.Host, fmt.Sprint(req.Target.Port))

	dialer := &net.Dialer{Timeout: timeout}
	conn, err := dialer.Dial("tcp", address)
	if err != nil {
		resp.FailedStage = "tcp"
		resp.ErrorSummary = str(err.Error())
		resp.DurationMS = uint64(time.Since(started).Milliseconds())
		return resp
	}
	defer conn.Close()

	counted := &countingConn{Conn: conn}
	// The lab deliberately runs targets with self-signed and PQC certificates; verification
	// is the target's business, not the prober's.
	tlsConn := tls.Client(counted, &tls.Config{
		ServerName:         sni,
		InsecureSkipVerify: true,
		CurvePreferences:   curves,
		MinVersion:         tls.VersionTLS12,
	})
	_ = tlsConn.SetDeadline(time.Now().Add(timeout))

	if err := tlsConn.Handshake(); err != nil {
		resp.Alert = alertFromError(err)
		resp.ErrorSummary = str(err.Error())
		if resp.Alert != nil {
			resp.Messages = append(resp.Messages, handshakeMessage{
				Index: 1, Direction: "inbound", Kind: "Alert",
				Fields: []messageField{
					{Label: "Level", Values: []string{resp.Alert.Level}},
					{Label: "Description", Values: []string{resp.Alert.Description}},
				},
			})
		}
		resp.Evidence = evidence{BytesSent: counted.written, BytesReceived: counted.read}
		resp.DurationMS = uint64(time.Since(started).Milliseconds())
		return resp
	}

	state := tlsConn.ConnectionState()
	resp.HandshakeSuccess = true
	resp.Negotiated = negotiated{
		TLSVersion:  str(versionName(state.Version)),
		CipherSuite: str(tls.CipherSuiteName(state.CipherSuite)),
		Group:       str(groupName(state.CurveID)),
	}
	resp.Messages = append(resp.Messages, handshakeMessage{
		Index: 1, Direction: "inbound", Kind: "ServerHello",
		Fields: []messageField{
			{Label: "Version", Values: []string{versionName(state.Version)}},
			{Label: "Cipher Suite", Values: []string{tls.CipherSuiteName(state.CipherSuite)}},
			{Label: "Key Exchange Group", Values: []string{groupName(state.CurveID)}},
		},
	})
	for _, cert := range state.PeerCertificates {
		notBefore := cert.NotBefore.UTC().Format(time.RFC3339)
		notAfter := cert.NotAfter.UTC().Format(time.RFC3339)
		resp.CertChain = append(resp.CertChain, certificate{
			Subject:            cert.Subject.String(),
			Issuer:             cert.Issuer.String(),
			NotBefore:          &notBefore,
			NotAfter:           &notAfter,
			SignatureAlgorithm: str(cert.SignatureAlgorithm.String()),
			PublicKeyAlgorithm: str(cert.PublicKeyAlgorithm.String()),
		})
	}

	resp.FailedStage = "http_request"
	if outcome, raw, err := doHTTP(tlsConn, req, sni); err == nil {
		resp.HTTP = outcome
		resp.FailedStage = "complete"
		if req.Capture.RawRecords {
			encoded := base64.StdEncoding.EncodeToString(raw)
			resp.Evidence.UDSDumpB64 = &encoded
		}
	} else {
		resp.ErrorSummary = str(err.Error())
	}

	resp.Evidence.BytesSent = counted.written
	resp.Evidence.BytesReceived = counted.read
	resp.DurationMS = uint64(time.Since(started).Milliseconds())
	return resp
}

func doHTTP(conn *tls.Conn, req probeRequest, sni string) (*httpOutcome, []byte, error) {
	path := req.Target.HTTPPath
	if path == "" {
		path = "/"
	}
	request := fmt.Sprintf("GET %s HTTP/1.1\r\nHost: %s\r\nUser-Agent: pqcas-probe\r\nConnection: close\r\nAccept: */*\r\n\r\n", path, sni)
	if _, err := conn.Write([]byte(request)); err != nil {
		return nil, nil, err
	}
	raw, err := io.ReadAll(io.LimitReader(conn, 64*1024))
	if err != nil && len(raw) == 0 {
		return nil, nil, err
	}

	text := string(raw)
	head, body, _ := strings.Cut(text, "\r\n\r\n")
	lines := strings.Split(head, "\r\n")
	if len(lines) == 0 || !strings.HasPrefix(lines[0], "HTTP/") {
		return nil, raw, fmt.Errorf("no HTTP response")
	}
	parts := strings.Fields(lines[0])
	status := 0
	if len(parts) > 1 {
		fmt.Sscanf(parts[1], "%d", &status)
	}
	headers := make([]messageField, 0, len(lines)-1)
	for _, line := range lines[1:] {
		if name, value, ok := strings.Cut(line, ":"); ok {
			headers = append(headers, messageField{Label: strings.TrimSpace(name), Values: []string{strings.TrimSpace(value)}})
		}
	}
	var excerpt *string
	if trimmed := strings.TrimSpace(body); trimmed != "" {
		if len(trimmed) > 512 {
			trimmed = trimmed[:512]
		}
		excerpt = &trimmed
	}
	return &httpOutcome{Status: status, Headers: headers, BodyExcerpt: excerpt}, raw, nil
}

// countingConn gives the report the same sent/received byte counts the other stacks print.
type countingConn struct {
	net.Conn
	read    uint64
	written uint64
}

func (c *countingConn) Read(b []byte) (int, error) {
	n, err := c.Conn.Read(b)
	c.read += uint64(n)
	return n, err
}

func (c *countingConn) Write(b []byte) (int, error) {
	n, err := c.Conn.Write(b)
	c.written += uint64(n)
	return n, err
}

func main() {
	adapterName := env("ADAPTER_NAME", "go")
	version := env("ADAPTER_VERSION", "go"+strings.TrimPrefix(runtimeVersion(), "go"))
	bind := env("ADAPTER_BIND", "0.0.0.0:9100")

	mux := http.NewServeMux()
	mux.HandleFunc("/healthz", func(w http.ResponseWriter, r *http.Request) {
		writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
	})
	mux.HandleFunc("/capabilities", func(w http.ResponseWriter, r *http.Request) {
		writeJSON(w, http.StatusOK, capabilities{
			Adapter:     adapterName,
			Version:     version,
			KEMGroups:   supportedGroups(),
			SigAlgs:     supportedSigAlgs(),
			TLSVersions: []string{"TLSv1.3", "TLSv1.2"},
		})
	})
	mux.HandleFunc("/probe", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			writeJSON(w, http.StatusMethodNotAllowed, map[string]string{"error": "POST only"})
			return
		}
		var req probeRequest
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			writeJSON(w, http.StatusBadRequest, map[string]string{"error": err.Error()})
			return
		}
		if strings.TrimSpace(req.Target.Host) == "" {
			writeJSON(w, http.StatusBadRequest, map[string]string{"error": "target host is required"})
			return
		}
		writeJSON(w, http.StatusOK, probe(req, adapterName))
	})

	server := &http.Server{
		Addr:              bind,
		Handler:           mux,
		ReadHeaderTimeout: 10 * time.Second,
	}
	log.Printf("adapter-go listening on %s", bind)
	if err := server.ListenAndServe(); err != nil {
		log.Fatal(err)
	}
}

func writeJSON(w http.ResponseWriter, status int, body any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(body)
}
