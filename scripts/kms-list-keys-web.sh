curl -X POST https://kms.aastar.io/ListKeys \
-H "Content-Type: application/json" \
-H "x-amz-target: TrentService.ListKeys" \
-d '{}'


curl -X POST https://kms.aastar.io/CreateKey \
-H "Content-Type: application/json" \
-H "x-amz-target: TrentService.CreateKey" \
-d '{
    "Description": "Test wallet",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS"
}'

curl -X POST https://kms.aastar.io/ListKeys \
-H "Content-Type: application/json" \
-H "x-amz-target: TrentService.ListKeys" \
-d '{}'
