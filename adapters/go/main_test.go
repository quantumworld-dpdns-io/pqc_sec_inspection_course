package main

import (
	"crypto/tls"
	"errors"
	"testing"
)

func TestGroupNameCoversPQCHybrid(t *testing.T) {
	if got := groupName(tls.X25519MLKEM768); got != "X25519MLKEM768" && got != "x25519MLKEM768" {
		t.Fatalf("unexpected group name %q", got)
	}
	if got := groupName(tls.CurveP256); got != "P-256" && got != "secp256r1" {
		t.Fatalf("unexpected group name %q", got)
	}
}

func TestAlertFromErrorMapsCommonFailures(t *testing.T) {
	cases := map[string]string{
		"remote error: tls: handshake failure": "handshake_failure",
		"local error: tls: bad record MAC":     "bad_record_mac",
		"tls: protocol version not supported":  "protocol_version",
	}
	for input, want := range cases {
		got := alertFromError(errors.New(input))
		if got == nil || got.Description != want {
			t.Fatalf("%q -> %+v, want %s", input, got, want)
		}
	}
	if alertFromError(errors.New("connection reset by peer")) != nil {
		t.Fatal("unrelated errors should not be reported as TLS alerts")
	}
}

func TestProbeRejectsGroupsGoDoesNotImplement(t *testing.T) {
	resp := probe(probeRequest{
		Target:    target{Host: "127.0.0.1", Port: 1},
		KEMGroups: []string{"mceliece460896"},
		TimeoutMS: 100,
	}, "go")

	if resp.HandshakeSuccess {
		t.Fatal("expected failure")
	}
	if resp.ErrorSummary == nil || resp.FailedStage != "tls_handshake" {
		t.Fatalf("unexpected response %+v", resp)
	}
}

func TestClientHelloAlwaysRecordsWhatWasOffered(t *testing.T) {
	msg := clientHello(probeRequest{KEMGroups: []string{"X25519MLKEM768"}, SigAlgs: []string{"mldsa65"}})
	if msg.Kind != "ClientHello" || len(msg.Fields) != 4 {
		t.Fatalf("unexpected message %+v", msg)
	}
	if msg.Fields[0].Values[0] != "X25519MLKEM768" {
		t.Fatalf("offered groups not recorded: %+v", msg.Fields[0])
	}
}
