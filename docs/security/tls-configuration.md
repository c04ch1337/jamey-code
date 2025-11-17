# TLS/HTTPS Configuration Guide

> **Navigation**: [Documentation Home](../README.md) > [Security](README.md) > TLS Configuration

This guide explains how to configure HTTPS with proper TLS security for production deployments of Digital Twin Jamey.

## Table of Contents

1. [Overview](#overview)
2. [Certificate Provisioning](#certificate-provisioning)
3. [Configuration](#configuration)
4. [Security Best Practices](#security-best-practices)
5. [Testing](#testing)
6. [Troubleshooting](#troubleshooting)

## Overview

Digital Twin Jamey supports secure HTTPS connections with:

- **TLS 1.2 and 1.3** support (TLS 1.3 recommended)
- **Strong cipher suites** configured by default
- **HSTS (HTTP Strict Transport Security)** headers
- **Security headers** (X-Content-Type-Options, X-Frame-Options, CSP)
- **Certificate validation** and proper error handling
- **HTTP to HTTPS redirection** in production

## Certificate Provisioning

### Option 1: Let's Encrypt (Recommended for Production)

Let's Encrypt provides free, automated SSL/TLS certificates. Use Certbot to obtain and manage certificates:

#### Installation

**Ubuntu/Debian:**
```bash
sudo apt-get update
sudo apt-get install certbot
```

**CentOS/RHEL:**
```bash
sudo yum install certbot
```

**macOS:**
```bash
brew install certbot
```

#### Obtaining Certificates

1. **Standalone mode** (requires port 80 to be available):
```bash
sudo certbot certonly --standalone -d your-domain.com -d www.your-domain.com
```

2. **Webroot mode** (if you have a web server running):
```bash
sudo certbot certonly --webroot -w /var/www/html -d your-domain.com
```

3. **DNS challenge** (for wildcard certificates):
```bash
sudo certbot certonly --manual --preferred-challenges dns -d your-domain.com -d *.your-domain.com
```

Certificates will be stored in:
- Certificate: `/etc/letsencrypt/live/your-domain.com/fullchain.pem`
- Private Key: `/etc/letsencrypt/live/your-domain.com/privkey.pem`
- CA Bundle: `/etc/letsencrypt/live/your-domain.com/chain.pem`

#### Automatic Renewal

Let's Encrypt certificates expire after 90 days. Set up automatic renewal:

```bash
# Test renewal
sudo certbot renew --dry-run

# Add to crontab for automatic renewal
sudo crontab -e
```

Add this line to renew twice daily:
```
0 0,12 * * * certbot renew --quiet --post-hook "systemctl restart jamey"
```

### Option 2: Commercial Certificate Authority

Purchase certificates from providers like:
- DigiCert
- GlobalSign
- Sectigo (formerly Comodo)
- GoDaddy

Follow your CA's instructions for:
1. Generating a Certificate Signing Request (CSR)
2. Validating domain ownership
3. Downloading and installing certificates

### Option 3: Self-Signed Certificates (Development Only)

**WARNING:** Self-signed certificates should NEVER be used in production.

#### Generate Self-Signed Certificate

```bash
# Create directory for certificates
mkdir -p ./certs

# Generate private key and certificate
openssl req -x509 -newkey rsa:4096 -keyout ./certs/localhost.key \
  -out ./certs/localhost.crt -days 365 -nodes \
  -subj "/C=US/ST=State/L=City/O=Organization/CN=localhost"

# Set proper permissions
chmod 600 ./certs/localhost.key
chmod 644 ./certs/localhost.crt
```

#### Trust Self-Signed Certificate (Development)

**macOS:**
```bash
sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain ./certs/localhost.crt
```

**Linux:**
```bash
sudo cp ./certs/localhost.crt /usr/local/share/ca-certificates/
sudo update-ca-certificates
```

**Windows:**
```powershell
Import-Certificate -FilePath .\certs\localhost.crt -CertStoreLocation Cert:\LocalMachine\Root
```

## Configuration

### Environment Variables

Configure TLS in your `config/production.env` file:

```bash
# Enable HTTPS
API_ENABLE_HTTPS=true
API_REDIRECT_HTTP_TO_HTTPS=true

# Port Configuration
API_HTTP_PORT=3000
API_HTTPS_PORT=3443

# Certificate Paths
API_TLS_CERT_PATH=/etc/letsencrypt/live/your-domain.com/fullchain.pem
API_TLS_KEY_PATH=/etc/letsencrypt/live/your-domain.com/privkey.pem
API_TLS_CA_CERT_PATH=/etc/letsencrypt/live/your-domain.com/chain.pem

# TLS Version (1.2 or 1.3)
API_TLS_MIN_VERSION=1.3

# HSTS Configuration
API_ENABLE_HSTS=true
API_HSTS_MAX_AGE=31536000
API_HSTS_INCLUDE_SUBDOMAINS=true
API_HSTS_PRELOAD=false

# CORS Configuration
ALLOWED_ORIGINS=https://your-domain.com,https://www.your-domain.com
ENABLE_CORS=false
```

### Configuration Options Explained

| Variable | Description | Default | Production Value |
|----------|-------------|---------|------------------|
| `API_ENABLE_HTTPS` | Enable HTTPS server | `false` | `true` |
| `API_REDIRECT_HTTP_TO_HTTPS` | Redirect HTTP to HTTPS | `false` | `true` |
| `API_HTTP_PORT` | HTTP port | `3000` | `80` or `3000` |
| `API_HTTPS_PORT` | HTTPS port | `3443` | `443` or `3443` |
| `API_TLS_CERT_PATH` | Path to certificate file | - | `/etc/letsencrypt/live/domain/fullchain.pem` |
| `API_TLS_KEY_PATH` | Path to private key | - | `/etc/letsencrypt/live/domain/privkey.pem` |
| `API_TLS_CA_CERT_PATH` | Path to CA bundle (optional) | - | `/etc/letsencrypt/live/domain/chain.pem` |
| `API_TLS_MIN_VERSION` | Minimum TLS version | `1.3` | `1.3` (or `1.2` if needed) |
| `API_ENABLE_HSTS` | Enable HSTS header | `true` | `true` |
| `API_HSTS_MAX_AGE` | HSTS max-age in seconds | `31536000` | `31536000` (1 year) |
| `API_HSTS_INCLUDE_SUBDOMAINS` | Include subdomains in HSTS | `true` | `true` |
| `API_HSTS_PRELOAD` | Enable HSTS preload | `false` | `true` (after testing) |

### File Permissions

Ensure proper permissions for certificate files:

```bash
# Certificate can be world-readable
sudo chmod 644 /etc/letsencrypt/live/your-domain.com/fullchain.pem

# Private key must be restricted
sudo chmod 600 /etc/letsencrypt/live/your-domain.com/privkey.pem
sudo chown root:root /etc/letsencrypt/live/your-domain.com/privkey.pem
```

## Security Best Practices

### 1. TLS Version

- **Use TLS 1.3** when possible (best performance and security)
- **Minimum TLS 1.2** for compatibility with older clients
- **Never use TLS 1.0 or 1.1** (deprecated and insecure)

### 2. HSTS Configuration

Enable HSTS to prevent protocol downgrade attacks:

```bash
API_ENABLE_HSTS=true
API_HSTS_MAX_AGE=31536000  # 1 year
API_HSTS_INCLUDE_SUBDOMAINS=true
```

**HSTS Preload:**
- Only enable after thoroughly testing HTTPS
- Submit your domain to [hstspreload.org](https://hstspreload.org/)
- Cannot be easily reversed

### 3. Certificate Management

- **Automate renewal** to prevent expiration
- **Monitor expiration dates** (set alerts for 30 days before)
- **Use strong key sizes** (minimum 2048-bit RSA or 256-bit ECC)
- **Keep private keys secure** (never commit to version control)

### 4. Security Headers

The application automatically sets these security headers:

- `Strict-Transport-Security`: Enforces HTTPS
- `X-Content-Type-Options: nosniff`: Prevents MIME sniffing
- `X-Frame-Options: DENY`: Prevents clickjacking
- `X-XSS-Protection: 1; mode=block`: XSS protection
- `Content-Security-Policy`: Restricts resource loading

### 5. Cipher Suites

The application uses Rustls with safe default cipher suites:

- `TLS_AES_256_GCM_SHA384` (TLS 1.3)
- `TLS_AES_128_GCM_SHA256` (TLS 1.3)
- `TLS_CHACHA20_POLY1305_SHA256` (TLS 1.3)
- `TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384` (TLS 1.2)
- `TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256` (TLS 1.2)

## Testing

### 1. Local Testing

Test HTTPS locally with self-signed certificates:

```bash
# Start the server
cargo run --release

# Test with curl (ignore certificate validation for self-signed)
curl -k https://localhost:3443/health

# Test with proper certificate validation
curl --cacert ./certs/localhost.crt https://localhost:3443/health
```

### 2. SSL Labs Test

Test your production deployment:

1. Visit [SSL Labs SSL Test](https://www.ssllabs.com/ssltest/)
2. Enter your domain name
3. Wait for the analysis to complete
4. Aim for an **A+ rating**

### 3. Certificate Validation

Verify certificate installation:

```bash
# Check certificate details
openssl s_client -connect your-domain.com:443 -servername your-domain.com

# Verify certificate chain
openssl s_client -connect your-domain.com:443 -showcerts

# Check certificate expiration
echo | openssl s_client -connect your-domain.com:443 2>/dev/null | \
  openssl x509 -noout -dates
```

### 4. HSTS Testing

Verify HSTS header:

```bash
curl -I https://your-domain.com | grep -i strict-transport-security
```

Expected output:
```
Strict-Transport-Security: max-age=31536000; includeSubDomains
```

### 5. TLS Version Testing

Test TLS version support:

```bash
# Test TLS 1.3
openssl s_client -connect your-domain.com:443 -tls1_3

# Test TLS 1.2
openssl s_client -connect your-domain.com:443 -tls1_2

# Test TLS 1.1 (should fail)
openssl s_client -connect your-domain.com:443 -tls1_1
```

## Troubleshooting

### Certificate Not Found

**Error:** `Certificate file not found: /path/to/cert.pem`

**Solution:**
1. Verify the file path in your configuration
2. Check file permissions
3. Ensure the certificate file exists:
   ```bash
   ls -la /etc/letsencrypt/live/your-domain.com/
   ```

### Permission Denied

**Error:** `Failed to load private key: Permission denied`

**Solution:**
```bash
# Fix permissions
sudo chmod 600 /etc/letsencrypt/live/your-domain.com/privkey.pem

# Or run the application with appropriate permissions
sudo -u jamey ./jamey-runtime
```

### Certificate Expired

**Error:** `Certificate has expired`

**Solution:**
```bash
# Renew certificate
sudo certbot renew

# Restart the application
sudo systemctl restart jamey
```

### Mixed Content Warnings

**Issue:** Browser shows mixed content warnings

**Solution:**
1. Ensure all resources load over HTTPS
2. Update `ALLOWED_ORIGINS` to use HTTPS URLs
3. Enable `API_REDIRECT_HTTP_TO_HTTPS=true`

### Port Already in Use

**Error:** `Address already in use (port 443)`

**Solution:**
```bash
# Check what's using the port
sudo lsof -i :443

# Stop conflicting service
sudo systemctl stop nginx  # or apache2

# Or use a different port
API_HTTPS_PORT=3443
```

### Browser Certificate Warnings

**Issue:** Browser shows "Your connection is not private"

**For Production:**
- Verify certificate is from a trusted CA
- Check certificate matches domain name
- Ensure certificate chain is complete

**For Development:**
- This is expected with self-signed certificates
- Add certificate to system trust store (see above)
- Or use browser's "Proceed anyway" option

## Production Deployment Checklist

- [ ] Obtain valid SSL/TLS certificate from trusted CA
- [ ] Configure certificate paths in `config/production.env`
- [ ] Set `API_ENABLE_HTTPS=true`
- [ ] Set `API_REDIRECT_HTTP_TO_HTTPS=true`
- [ ] Use TLS 1.3 or minimum TLS 1.2
- [ ] Enable HSTS with appropriate max-age
- [ ] Configure automatic certificate renewal
- [ ] Set up monitoring for certificate expiration
- [ ] Test with SSL Labs (aim for A+ rating)
- [ ] Verify all security headers are present
- [ ] Update firewall rules for HTTPS port
- [ ] Update DNS records if needed
- [ ] Test HTTP to HTTPS redirection
- [ ] Verify CORS configuration for HTTPS origins
- [ ] Document certificate renewal procedures
- [ ] Set up alerts for certificate expiration

## Related Documentation

- [Security Overview](README.md) - Overall security architecture
- [Log Security](log-security.md) - Secure logging practices
- [TA-QR Architecture](ta-qr/architecture.md) - Quantum-resistant cryptography
- [Operations Guide](../operations/README.md) - Deployment procedures

## Additional Resources

- [Mozilla SSL Configuration Generator](https://ssl-config.mozilla.org/)
- [Let's Encrypt Documentation](https://letsencrypt.org/docs/)
- [OWASP TLS Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Transport_Layer_Protection_Cheat_Sheet.html)
- [Rustls Documentation](https://docs.rs/rustls/)
- [HSTS Preload List](https://hstspreload.org/)

---

**Last Updated**: 2025-11-17
**Status**: ✅ Complete
**Category**: Security