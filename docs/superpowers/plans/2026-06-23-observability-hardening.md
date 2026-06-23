# Observability Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Production-readiness improvements across logging, metrics, health checking, and request protection.

**Architecture:** Four independent improvements layered on top of existing Axum/Tower stack. Each touches a different concern and can be reviewed/merged independently. Order: body limits (safety) → health check (ops) → logging (insight) → metrics (depth).

**Tech Stack:** Rust 1.75+, Axum 0.8, Tower 0.4, tower-http 0.6, tracing 0.1, metrics 0.24, metrics-exporter-prometheus 0.16.

---
