#!/bin/bash


# if lnd enabled, attempt to connect
if [[ ! -z "${LND_RPC_AUTHORITY}" ]]; then
    exec opuza \
	 --directory $FILES_DIR \
	 --http-port $OPUZA_PORT \
	 --lnd-rpc-authority $LND_RPC_AUTHORITY \
	 --lnd-rpc-cert-path $TLS_CERT_PATH \
	 --lnd-rpc-macaroon-path $INVOICES_MACAROON_PATH
# else run simple server
else
    exec opuza \
	 --directory $FILES_DIR \
	 --http-port $OPUZA_PORT
fi
