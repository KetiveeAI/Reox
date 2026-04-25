# Reox APT Repository

Add this repository to your sources:

```bash
curl -fsSL https://ketiveeai.github.io/Reox/reox.gpg | sudo gpg --dearmor -o /usr/share/keyrings/reox.gpg
echo "deb [signed-by=/usr/share/keyrings/reox.gpg] https://ketiveeai.github.io/Reox stable main" | sudo tee /etc/apt/sources.list.d/reox.list
sudo apt update
sudo apt install reoxc
```
